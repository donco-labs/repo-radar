use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanConfig {
    pub ignored_directories: Vec<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            ignored_directories: [".git", "target", "node_modules"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanWarning {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub files: usize,
    pub bytes: u64,
    pub by_extension: BTreeMap<String, usize>,
    pub largest_files: Vec<FileEntry>,
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

pub fn scan(root: &Path, config: &ScanConfig) -> io::Result<ScanReport> {
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} is not a directory", root.display()),
        ));
    }

    let mut report = ScanReport::default();
    scan_directory(root, root, config, &mut report);
    report.largest_files.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(report)
}

fn scan_directory(root: &Path, directory: &Path, config: &ScanConfig, report: &mut ScanReport) {
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
            scan_directory(root, &path, config, report);
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
            .unwrap_or("[no extension]")
            .to_ascii_lowercase();

        report.files += 1;
        report.bytes += bytes;
        *report.by_extension.entry(extension).or_default() += 1;
        report.largest_files.push(FileEntry {
            path: relative_path,
            bytes,
        });
    }
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
}
