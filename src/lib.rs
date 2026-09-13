//! Repo Radar's scan engine and reporting model.
//!
//! This crate reads a repository and produces a [`ScanReport`]: it never
//! writes into the tree it scans, never mutates Git state, and never
//! executes anything found in it. `docs/specs/000-safety-invariants.md`
//! states the full set of guarantees and outranks every other document in
//! this repository, including `SPEC.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::unwrap_used, clippy::expect_used)]
#![warn(clippy::todo, clippy::unimplemented)]

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub mod analysis;
mod languages;
pub mod render;

pub use analysis::cargo::{
    CargoDependency, CargoLock, CargoManifest, DependencyKind, DependencySource,
};
pub use analysis::git::{DayCount, GitActivity, GitStatus};
pub use languages::LANGUAGE_TABLE_VERSION;

/// What a scan does and does not look at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanConfig {
    /// Directory names — not paths — skipped entirely during traversal.
    /// The default set is `.git`, `target`, and `node_modules`.
    pub ignored_directories: Vec<String>,
    /// Read file contents to count lines. Defaults to true.
    pub count_lines: bool,
    /// Run the Git analyses. Defaults to true.
    pub read_git: bool,
    /// Days back the activity window covers. Defaults to 30.
    pub activity_window_days: u32,
    /// Read Cargo manifests. Defaults to true.
    pub read_cargo: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            ignored_directories: [".git", "target", "node_modules"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            count_lines: true,
            read_git: true,
            activity_window_days: 30,
            read_cargo: true,
        }
    }
}

/// A single counted file: its repository-relative path and size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileEntry {
    /// Repository-relative.
    pub path: PathBuf,
    /// File size in bytes, as reported by `stat`.
    pub bytes: u64,
}

/// Aggregate totals for one language, as determined by file extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanguageStat {
    /// The language name, or `[no extension]` / `[unrecognized]` for files
    /// the extension table does not map.
    pub language: String,
    /// File count contributing to this language.
    pub files: usize,
    /// Byte total across those files.
    pub bytes: u64,
    /// Zero when line counting was disabled or the files were binary.
    pub lines: u64,
}

/// Aggregate totals for one directory, including every descendant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DirectoryEntry {
    /// Repository-relative. Never the root.
    pub path: PathBuf,
    /// Aggregate over the directory and all its descendants.
    pub files: usize,
    /// Aggregate over the directory and all its descendants.
    pub bytes: u64,
}

/// The result of an analysis that may not have been able to run.
///
/// Invariant I10 requires that an analysis which did not run reports
/// `not evaluated` rather than a plausible default. Making that a type rather
/// than a convention means a caller cannot render a zero where it meant
/// "unknown" — there is no zero to reach for, because there is no `T`.
///
/// ```
/// use repo_radar::{Analysis, NotEvaluated};
///
/// let ran: Analysis<u64> = Analysis::Ran(42);
/// assert_eq!(ran.ran(), Some(&42));
///
/// let skipped: Analysis<u64> = Analysis::NotEvaluated(NotEvaluated::Disabled);
/// assert_eq!(skipped.ran(), None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Analysis<T> {
    /// The analysis ran and produced this result.
    Ran(T),
    /// The analysis did not run, for the stated reason.
    NotEvaluated(NotEvaluated),
}

/// Why an analysis did not run.
///
/// The four variants are the four things a user needs told apart. Collapsing
/// them into a single "unavailable" would put "you switched this off" and "we
/// tried to read this and failed" in the same bucket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotEvaluated {
    /// Switched off for this invocation, e.g. `--no-lines`.
    Disabled,
    /// The input does not exist here, e.g. not a Git worktree.
    InputUnavailable(String),
    /// Recognized but not implemented, e.g. an unimplemented agent adapter.
    Unsupported(String),
    /// The input existed and could not be understood.
    Failed(String),
}

impl<T> Default for Analysis<T> {
    fn default() -> Self {
        Self::NotEvaluated(NotEvaluated::Disabled)
    }
}

impl<T> Analysis<T> {
    /// The result, or `None` if the analysis did not run.
    pub fn ran(&self) -> Option<&T> {
        match self {
            Self::Ran(value) => Some(value),
            Self::NotEvaluated(_) => None,
        }
    }
}

impl NotEvaluated {
    /// The stable wire token for this reason: `disabled`,
    /// `input_unavailable`, `unsupported`, or `failed`.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::InputUnavailable(_) => "input_unavailable",
            Self::Unsupported(_) => "unsupported",
            Self::Failed(_) => "failed",
        }
    }

    /// The specific input this reason names, when it names one.
    /// `Disabled` carries none.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::Disabled => None,
            Self::InputUnavailable(detail) | Self::Unsupported(detail) | Self::Failed(detail) => {
                Some(detail)
            }
        }
    }
}

/// Serializes as `{ "evaluated": bool, "reason"?: ..., "detail"?: ..., ...T's
/// own fields }`, per `docs/specs/002-structured-output.md`.
///
/// `#[serde(flatten)]` requires `T` to serialize as a map; every analysis
/// result in this crate is a struct, so that holds.
impl<T: Serialize + Default> Serialize for Analysis<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Wire<'a, T: Serialize> {
            evaluated: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            reason: Option<&'static str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            detail: Option<&'a str>,
            #[serde(flatten)]
            value: &'a T,
        }

        // A `NotEvaluated` analysis has no `T`, but the version 1 contract
        // requires the object keep its full field shape so a consumer's field
        // access never fails. The zero value stands in, and `evaluated: false`
        // is what marks those zeros as something other than measurements.
        let fallback;
        let wire = match self {
            Self::Ran(value) => Wire {
                evaluated: true,
                reason: None,
                detail: None,
                value,
            },
            Self::NotEvaluated(not_evaluated) => {
                fallback = T::default();
                Wire {
                    evaluated: false,
                    reason: Some(not_evaluated.reason()),
                    detail: not_evaluated.detail(),
                    value: &fallback,
                }
            }
        };
        wire.serialize(serializer)
    }
}

/// Line counting results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LineCounts {
    /// Total lines across all text files.
    pub lines: u64,
    /// Files read as text.
    pub text_files: usize,
    /// Files skipped as binary by the NUL-byte heuristic.
    pub binary_files: usize,
    /// Files that stat succeeded on but could not be opened.
    pub unreadable_files: usize,
}

/// A non-fatal problem encountered while scanning, e.g. a directory entry
/// that could not be read or a file that disappeared mid-scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanWarning {
    /// The path the problem occurred on.
    pub path: PathBuf,
    /// The underlying error, as text.
    pub message: String,
}

/// The full result of a scan: every fact `repo-radar` has about the
/// repository, in the shape every surface (text, JSON, HTML, `serve`, the
/// TUI) renders from.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ScanReport {
    /// Total files counted.
    pub files: usize,
    /// Total bytes across all counted files.
    pub bytes: u64,
    /// File count by extension, lower-cased.
    pub by_extension: BTreeMap<String, usize>,
    /// Aggregate totals by language, sorted by bytes descending.
    pub by_language: Vec<LanguageStat>,
    /// The largest files, sorted by bytes descending.
    pub largest_files: Vec<FileEntry>,
    /// The largest directories by aggregate bytes descending. The
    /// repository root is excluded, since it is trivially first and every
    /// scan has one.
    pub largest_directories: Vec<DirectoryEntry>,
    /// Line-counting results, or why they were not produced.
    pub lines: Analysis<LineCounts>,
    /// Git status counts, or why they were not produced.
    pub git_status: Analysis<GitStatus>,
    /// Recent commit activity, or why it was not produced.
    pub git_activity: Analysis<GitActivity>,
    /// What `Cargo.toml` declares, or why it was not read.
    pub cargo_manifest: Analysis<CargoManifest>,
    /// What `Cargo.lock` resolved to, or why it was not read.
    pub cargo_lock: Analysis<CargoLock>,
    /// Non-fatal problems encountered while scanning.
    pub warnings: Vec<ScanWarning>,
}

/// Replaces characters that a terminal would interpret as commands.
///
/// Repository content — file names, branch names, commit messages — is
/// untrusted input. Written to a terminal unchanged, an embedded escape
/// sequence can recolor, reposition, or erase output, letting a hostile
/// repository forge or hide part of a report. This upholds invariant I4 of
/// `docs/specs/000-safety-invariants.md`.
///
/// C0 controls, `DEL`, and the C1 range are replaced with U+FFFD so the
/// presence of the character stays visible. Tab and newline are also replaced,
/// because a name containing either can fake a column or an entire row.
pub fn sanitize_for_terminal(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() || ('\u{80}'..='\u{9f}').contains(&character) {
                char::REPLACEMENT_CHARACTER
            } else {
                character
            }
        })
        .collect()
}

/// Renders a path for terminal display with control characters neutralized.
///
/// Lossy conversion is deliberate: a non-UTF-8 path must still be reportable.
pub fn display_path(path: &Path) -> String {
    sanitize_for_terminal(&path.to_string_lossy())
}

/// Scans `root` and produces a [`ScanReport`].
///
/// Traversal never follows symlinks and never leaves `root` (invariant I8),
/// and nothing under `root` is created, modified, deleted, or renamed
/// (invariant I1) — on any path, including the error paths below.
///
/// # Errors
///
/// Returns an error if `root` is not a directory.
///
/// ```
/// use repo_radar::{ScanConfig, scan};
///
/// let report = scan(std::path::Path::new("."), &ScanConfig::default())
///     .expect(". should be a directory");
/// assert!(report.files > 0);
/// ```
pub fn scan(root: &Path, config: &ScanConfig) -> io::Result<ScanReport> {
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} is not a directory", root.display()),
        ));
    }

    let mut report = ScanReport::default();
    let mut line_counts = LineCounts::default();

    let mut directories: BTreeMap<PathBuf, DirectoryEntry> = BTreeMap::new();
    let mut languages: BTreeMap<String, LanguageStat> = BTreeMap::new();

    scan_directory(
        root,
        root,
        config,
        &mut report,
        &mut line_counts,
        &mut directories,
        &mut languages,
    );

    report.lines = if config.count_lines {
        Analysis::Ran(line_counts)
    } else {
        Analysis::NotEvaluated(NotEvaluated::Disabled)
    };

    (report.git_status, report.git_activity) = analysis::git::analyze(root, config);
    (report.cargo_manifest, report.cargo_lock) = analysis::cargo::analyze(root, config);

    report.largest_files.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.path.cmp(&right.path))
    });

    // The root's aggregate (keyed by the empty path) is the check for
    // criterion 2 — it must equal `report.bytes` — but it would be trivially
    // first and say nothing in a list of directories, so it is dropped here.
    report.largest_directories = directories
        .into_values()
        .filter(|entry| !entry.path.as_os_str().is_empty())
        .collect();
    report.largest_directories.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.path.cmp(&right.path))
    });

    report.by_language = languages.into_values().collect();
    report.by_language.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.language.cmp(&right.language))
    });

    Ok(report)
}

fn scan_directory(
    root: &Path,
    directory: &Path,
    config: &ScanConfig,
    report: &mut ScanReport,
    line_counts: &mut LineCounts,
    directories: &mut BTreeMap<PathBuf, DirectoryEntry>,
    languages: &mut BTreeMap<String, LanguageStat>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            add_warning(report, directory.to_path_buf(), error);
            return;
        }
    };

    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry),
            Err(error) => add_warning(report, directory.to_path_buf(), error),
        }
    }
    paths.sort_by_key(|entry| entry.path());

    for entry in paths {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                add_warning(report, path, error);
                continue;
            }
        };

        if file_type.is_symlink() || should_skip(&path, config) {
            continue;
        }

        if file_type.is_dir() {
            scan_directory(
                root,
                &path,
                config,
                report,
                line_counts,
                directories,
                languages,
            );
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let bytes = match entry.metadata() {
            Ok(metadata) => metadata.len(),
            Err(error) => {
                add_warning(report, path, error);
                continue;
            }
        };
        let relative_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or(languages::NO_EXTENSION)
            .to_ascii_lowercase();

        report.files += 1;
        report.bytes += bytes;
        *report.by_extension.entry(extension.clone()).or_default() += 1;

        let mut file_lines = 0u64;
        if config.count_lines {
            match count_lines(&path) {
                FileText::Text { lines } => {
                    line_counts.lines += lines;
                    line_counts.text_files += 1;
                    file_lines = lines;
                }
                FileText::Binary => {
                    line_counts.binary_files += 1;
                }
                FileText::Unreadable => {
                    line_counts.unreadable_files += 1;
                    add_warning(
                        report,
                        path.clone(),
                        io::Error::other("could not read file to count lines"),
                    );
                }
            }
        }

        let language_name = match languages::language_for_extension(&extension) {
            Some(language) => language.to_owned(),
            None if extension == languages::NO_EXTENSION => languages::NO_EXTENSION.to_owned(),
            None => languages::UNRECOGNIZED.to_owned(),
        };
        let language_stat =
            languages
                .entry(language_name.clone())
                .or_insert_with(|| LanguageStat {
                    language: language_name,
                    files: 0,
                    bytes: 0,
                    lines: 0,
                });
        language_stat.files += 1;
        language_stat.bytes += bytes;
        language_stat.lines += file_lines;

        accumulate_directory(directories, &relative_path, bytes);

        report.largest_files.push(FileEntry {
            path: relative_path,
            bytes,
        });
    }
}

/// Adds `bytes` and a file count to every ancestor directory of
/// `relative_path`, including the repository root.
///
/// The root is represented by the empty path (`PathBuf::from("")`), the
/// natural last element of [`Path::ancestors`], so no sentinel string needs
/// inventing. `scan` filters that entry out of `largest_directories`; this
/// function does not, so its root total stays available as the check for
/// spec 003 criterion 2.
fn accumulate_directory(
    directories: &mut BTreeMap<PathBuf, DirectoryEntry>,
    relative_path: &Path,
    bytes: u64,
) {
    // `ancestors()` yields the path itself first; skip it, since a file is
    // not a directory of itself.
    for ancestor in relative_path.ancestors().skip(1) {
        let entry = directories
            .entry(ancestor.to_path_buf())
            .or_insert_with(|| DirectoryEntry {
                path: ancestor.to_path_buf(),
                files: 0,
                bytes: 0,
            });
        entry.files += 1;
        entry.bytes += bytes;
    }
}

enum FileText {
    Text { lines: u64 },
    Binary,
    Unreadable,
}

/// Classifies a file as text or binary and counts its lines.
///
/// Binary detection is the NUL-byte heuristic spec 003 specifies: a NUL in
/// the first 8 KiB marks a file binary. It is the same cheap check Git and
/// `grep` use, and it is wrong on rare inputs — UTF-16 source reads as
/// binary, and a text file with one stray NUL byte reads as binary too. Full
/// UTF-8 validation of every file was rejected as disproportionately
/// expensive for the accuracy it buys.
///
/// The file is streamed through a fixed-size buffer rather than read in
/// full, so memory stays bounded even for a multi-gigabyte file (invariant
/// I9). Lines are `\n` occurrences, plus one when the file is non-empty and
/// does not end in a newline; an empty file is text with zero lines.
fn count_lines(path: &Path) -> FileText {
    const CHUNK_SIZE: usize = 64 * 1024;
    const BINARY_SCAN_WINDOW: usize = 8 * 1024;

    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return FileText::Unreadable,
    };

    let mut reader = io::BufReader::with_capacity(CHUNK_SIZE, file);
    let mut buffer = [0u8; CHUNK_SIZE];
    let mut total_read = 0usize;
    let mut lines = 0u64;
    let mut last_byte: Option<u8> = None;

    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(_) => return FileText::Unreadable,
        };
        let chunk = &buffer[..read];

        if total_read < BINARY_SCAN_WINDOW {
            let scan_end = (BINARY_SCAN_WINDOW - total_read).min(read);
            if chunk[..scan_end].contains(&0) {
                return FileText::Binary;
            }
        }

        lines += chunk.iter().filter(|&&byte| byte == b'\n').count() as u64;
        last_byte = Some(chunk[read - 1]);
        total_read += read;
    }

    if let Some(last) = last_byte
        && last != b'\n'
    {
        lines += 1;
    }

    FileText::Text { lines }
}

fn add_warning(report: &mut ScanReport, path: PathBuf, error: io::Error) {
    report.warnings.push(ScanWarning {
        path,
        message: error.to_string(),
    });
}

fn should_skip(path: &Path, config: &ScanConfig) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            config
                .ignored_directories
                .iter()
                .any(|ignored| ignored == name)
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use std::fs::{self, File};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after the Unix epoch")
                .as_nanos();
            let counter = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!("repo-radar-{suffix}-{counter}"));
            fs::create_dir(&root).expect("fixture root should be created");
            Self { root }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn scans_files_deterministically_and_normalizes_extensions() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.root.join("src")).unwrap();
        fs::create_dir(fixture.root.join("target")).unwrap();
        fs::write(fixture.root.join("src/b.RS"), b"123").unwrap();
        fs::write(fixture.root.join("README"), b"12").unwrap();
        fs::write(fixture.root.join("target/ignored.rs"), b"ignored").unwrap();

        let report = scan(&fixture.root, &ScanConfig::default()).unwrap();

        assert_eq!(report.files, 2);
        assert_eq!(report.bytes, 5);
        assert_eq!(report.by_extension.get("rs"), Some(&1));
        assert_eq!(report.by_extension.get("[no extension]"), Some(&1));
        assert_eq!(report.largest_files[0].path, PathBuf::from("src/b.RS"));
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn supports_additional_ignored_directories() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.root.join("generated")).unwrap();
        fs::write(fixture.root.join("generated/data.txt"), b"data").unwrap();

        let config = ScanConfig {
            ignored_directories: vec!["generated".to_owned()],
            ..ScanConfig::default()
        };

        assert_eq!(scan(&fixture.root, &config).unwrap().files, 0);
    }

    #[test]
    fn scans_empty_directories_without_warnings() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.root.join("empty")).unwrap();

        let report = scan(&fixture.root, &ScanConfig::default()).unwrap();

        assert_eq!(report.files, 0);
        assert_eq!(report.bytes, 0);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn records_warning_path_and_message() {
        let mut report = ScanReport::default();
        let path = PathBuf::from("unreadable");

        add_warning(
            &mut report,
            path.clone(),
            io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
        );

        assert_eq!(report.warnings[0].path, path);
        assert_eq!(report.warnings[0].message, "permission denied");
    }

    #[test]
    fn rejects_missing_and_file_roots() {
        let fixture = Fixture::new();
        let file = fixture.root.join("file.txt");
        File::create(&file).unwrap();

        assert!(scan(&fixture.root.join("missing"), &ScanConfig::default()).is_err());
        assert!(scan(&file, &ScanConfig::default()).is_err());
    }

    #[test]
    fn sanitizes_terminal_control_sequences() {
        assert_eq!(
            sanitize_for_terminal("plain/name.rs"),
            "plain/name.rs",
            "ordinary text must pass through unchanged"
        );
        assert_eq!(
            sanitize_for_terminal("evil\u{1b}[31mname"),
            "evil\u{fffd}[31mname",
            "the escape byte must not survive, but the rest of the name must"
        );
        for hostile in ["a\u{7}b", "a\u{7f}b", "a\u{9b}b", "a\tb", "a\nb", "a\rb"] {
            let sanitized = sanitize_for_terminal(hostile);
            assert!(
                !sanitized.chars().any(|character| character.is_control()),
                "control character survived sanitizing {hostile:?} into {sanitized:?}"
            );
            assert_eq!(sanitized.chars().count(), 3, "length must be preserved");
        }
    }

    #[test]
    fn sanitizing_preserves_non_ascii_text() {
        for text in ["café/ünïcode.rs", "日本語", "emoji-🦀.rs"] {
            assert_eq!(sanitize_for_terminal(text), text);
        }
    }

    #[test]
    fn display_path_neutralizes_control_characters() {
        let rendered = display_path(Path::new("src/evil\u{1b}[2Kname.rs"));

        assert!(!rendered.contains('\u{1b}'));
        assert!(rendered.contains("[2Kname.rs"));
    }

    #[cfg(unix)]
    #[test]
    fn skips_symbolic_links() {
        let fixture = Fixture::new();
        let target = fixture.root.join("real.txt");
        let link = fixture.root.join("link.txt");
        fs::write(&target, b"real").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let report = scan(&fixture.root, &ScanConfig::default()).unwrap();

        assert_eq!(report.files, 1);
        assert_eq!(report.largest_files[0].path, PathBuf::from("real.txt"));
    }

    #[test]
    fn counts_lines_across_text_files() {
        let fixture = Fixture::new();
        let with_trailing_newline = fixture.root.join("with-trailing.txt");
        let without_trailing_newline = fixture.root.join("without-trailing.txt");
        fs::write(&with_trailing_newline, b"one\ntwo\nthree\n").unwrap();
        fs::write(&without_trailing_newline, b"one\ntwo\nthree").unwrap();

        let with_trailing_lines = match count_lines(&with_trailing_newline) {
            FileText::Text { lines } => lines,
            _ => panic!("a text file must not be classified binary or unreadable"),
        };
        let without_trailing_lines = match count_lines(&without_trailing_newline) {
            FileText::Text { lines } => lines,
            _ => panic!("a text file must not be classified binary or unreadable"),
        };

        assert_eq!(
            with_trailing_lines, 3,
            "a trailing newline adds no extra line"
        );
        assert_eq!(
            without_trailing_lines, 3,
            "a missing trailing newline still counts the final, unterminated line"
        );
        assert_eq!(with_trailing_lines + without_trailing_lines, 6);
    }

    #[test]
    fn treats_nul_bearing_files_as_binary() {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("data.bin"), b"\x00\x01\x02").unwrap();

        let report = scan(&fixture.root, &ScanConfig::default()).unwrap();
        let line_counts = report.lines.ran().expect("line counting was enabled");

        assert_eq!(line_counts.binary_files, 1);
        assert_eq!(line_counts.text_files, 0);
        assert_eq!(line_counts.lines, 0);
    }

    #[test]
    fn empty_file_is_text_with_zero_lines() {
        let fixture = Fixture::new();
        let path = fixture.root.join("empty.txt");
        fs::write(&path, b"").unwrap();

        match count_lines(&path) {
            FileText::Text { lines } => assert_eq!(lines, 0),
            _ => panic!("an empty file is text, not binary (spec 003, criterion 1)"),
        }
    }

    #[test]
    fn binary_files_still_contribute_bytes_and_extension() {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("data.bin"), b"\x00\x01\x02\x03").unwrap();

        let report = scan(&fixture.root, &ScanConfig::default()).unwrap();

        assert_eq!(report.bytes, 4);
        assert_eq!(report.by_extension.get("bin"), Some(&1));
    }

    #[test]
    fn directory_totals_sum_descendant_files() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.root.join("src/nested")).unwrap();
        fs::write(fixture.root.join("src/a.rs"), vec![b'a'; 10]).unwrap();
        fs::write(fixture.root.join("src/nested/b.rs"), vec![b'b'; 20]).unwrap();
        fs::write(fixture.root.join("top.rs"), vec![b't'; 5]).unwrap();

        let config = ScanConfig::default();
        let mut report = ScanReport::default();
        let mut directories = BTreeMap::new();
        let mut languages = BTreeMap::new();
        scan_directory(
            &fixture.root,
            &fixture.root,
            &config,
            &mut report,
            &mut LineCounts::default(),
            &mut directories,
            &mut languages,
        );

        let src = directories
            .get(Path::new("src"))
            .expect("src should aggregate its descendants");
        assert_eq!(src.bytes, 30);
        assert_eq!(src.files, 2);

        let nested = directories
            .get(Path::new("src/nested"))
            .expect("src/nested should aggregate only its own file");
        assert_eq!(nested.bytes, 20);
        assert_eq!(nested.files, 1);

        assert_eq!(report.bytes, 35);
        let root = directories
            .get(Path::new(""))
            .expect("the root is aggregated under the empty path");
        assert_eq!(
            root.bytes, report.bytes,
            "the root aggregate is the check for criterion 2"
        );
    }

    #[test]
    fn largest_directories_excludes_the_root() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.root.join("src")).unwrap();
        fs::write(fixture.root.join("src/a.rs"), b"12345").unwrap();

        let report = scan(&fixture.root, &ScanConfig::default()).unwrap();

        assert!(!report.largest_directories.is_empty());
        assert!(
            report
                .largest_directories
                .iter()
                .all(|entry| !entry.path.as_os_str().is_empty()),
            "the root must never appear in largest_directories"
        );
    }

    #[test]
    fn language_stats_group_and_sort_by_bytes() {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("a.rs"), vec![b'a'; 100]).unwrap();
        fs::write(fixture.root.join("b.rs"), vec![b'b'; 50]).unwrap();
        fs::write(fixture.root.join("c.md"), vec![b'c'; 10]).unwrap();

        let report = scan(&fixture.root, &ScanConfig::default()).unwrap();

        assert_eq!(report.by_language.len(), 2);
        assert_eq!(report.by_language[0].language, "Rust");
        assert_eq!(report.by_language[0].bytes, 150);
        assert_eq!(report.by_language[0].files, 2);
        assert_eq!(report.by_language[1].language, "Markdown");
        assert_eq!(report.by_language[1].bytes, 10);
    }

    #[test]
    fn unmapped_extensions_group_as_unrecognized() {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("a.rs"), vec![b'a'; 10]).unwrap();
        fs::write(fixture.root.join("b.xyz"), vec![b'b'; 20]).unwrap();

        let report = scan(&fixture.root, &ScanConfig::default()).unwrap();

        let unrecognized = report
            .by_language
            .iter()
            .find(|stat| stat.language == "[unrecognized]")
            .expect("an unmapped extension should group under [unrecognized]");
        assert_eq!(unrecognized.bytes, 20);

        let total: u64 = report.by_language.iter().map(|stat| stat.bytes).sum();
        assert_eq!(
            total, report.bytes,
            "by_language must reconcile with the byte total"
        );
    }

    #[test]
    fn disabling_line_counting_reports_not_evaluated() {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("a.rs"), b"one\ntwo\n").unwrap();

        let config = ScanConfig {
            count_lines: false,
            ..ScanConfig::default()
        };
        let report = scan(&fixture.root, &config).unwrap();

        assert_eq!(
            report.lines,
            Analysis::NotEvaluated(NotEvaluated::Disabled),
            "a disabled analysis must name its reason, not report a zero"
        );
    }

    #[test]
    fn analysis_that_ran_serializes_its_value_and_no_reason() {
        let analysis = Analysis::Ran(LineCounts {
            lines: 7,
            text_files: 1,
            ..Default::default()
        });

        let value = serde_json::to_value(&analysis).expect("Analysis should serialize");

        assert_eq!(value["evaluated"], true);
        assert_eq!(value["lines"], 7);
        assert!(
            value.get("reason").is_none(),
            "an evaluated analysis must not emit reason"
        );
        assert!(
            value.get("detail").is_none(),
            "an evaluated analysis must not emit detail"
        );
    }

    #[test]
    fn analysis_that_did_not_run_reports_reason_and_zeroed_fields() {
        let analysis: Analysis<LineCounts> = Analysis::NotEvaluated(NotEvaluated::Disabled);

        let value = serde_json::to_value(&analysis).expect("Analysis should serialize");

        assert_eq!(value["evaluated"], false);
        assert_eq!(value["reason"], "disabled");
        assert_eq!(value["lines"], 0);
        assert!(value.get("detail").is_none(), "Disabled carries no detail");
    }

    #[test]
    fn not_evaluated_detail_names_the_input() {
        let analysis: Analysis<LineCounts> = Analysis::NotEvaluated(NotEvaluated::Failed(
            "Cargo.toml is not valid TOML".to_owned(),
        ));

        let value = serde_json::to_value(&analysis).expect("Analysis should serialize");

        assert_eq!(value["reason"], "failed");
        assert_eq!(value["detail"], "Cargo.toml is not valid TOML");
    }

    #[test]
    fn every_not_evaluated_reason_has_a_distinct_token() {
        let reasons = [
            NotEvaluated::Disabled.reason(),
            NotEvaluated::InputUnavailable(String::new()).reason(),
            NotEvaluated::Unsupported(String::new()).reason(),
            NotEvaluated::Failed(String::new()).reason(),
        ];

        assert_eq!(
            reasons,
            ["disabled", "input_unavailable", "unsupported", "failed"]
        );

        let mut sorted: Vec<&str> = reasons.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            reasons.len(),
            "reason tokens must all be distinct"
        );
    }

    #[test]
    fn default_analysis_has_not_run() {
        assert!(Analysis::<LineCounts>::default().ran().is_none());
    }

    // Test 12 from the build sheet, `unreadable_file_warns_and_counts_without_aborting`,
    // needs a file that stats successfully but cannot be opened for reading.
    // `chmod 000` gives exactly that on Unix; there is no portable equivalent
    // to construct one on Windows, so the test is Unix-only rather than
    // skipped outright.
    #[cfg(unix)]
    #[test]
    fn unreadable_file_warns_and_counts_without_aborting() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new();
        let path = fixture.root.join("locked.rs");
        fs::write(&path, b"secret\n").unwrap();

        if fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).is_err() {
            eprintln!("skipping: permissions unavailable on this filesystem");
            return;
        }
        if File::open(&path).is_ok() {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).ok();
            eprintln!("skipping: running with privileges that ignore permissions");
            return;
        }

        let report = scan(&fixture.root, &ScanConfig::default()).unwrap();

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).ok();

        let line_counts = report.lines.ran().expect("line counting was enabled");
        assert_eq!(line_counts.unreadable_files, 1);
        assert!(
            report.warnings.iter().any(|warning| warning.path == path),
            "an unreadable file must be named in a warning, not silently skipped"
        );
    }
}
