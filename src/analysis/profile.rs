//! The repository's stated purpose: a manifest description, a README's
//! first substantive paragraph, or a non-placeholder `.git/description`.
//!
//! Spec 014 (project profile), parcel 6a. The precedence table is fixed and
//! versioned ([`PURPOSE_TABLE_VERSION`]), not a directory-order search: the
//! first source that yields a non-empty normalized statement wins, and later
//! sources are never consulted. Nothing is invented — a repository with none
//! of the seven sources reports [`crate::NotEvaluated::InputUnavailable`]
//! naming what was looked for, rather than an empty statement a surface
//! could render as a fact (invariant I10).
//!
//! ## Untrusted content (I4)
//!
//! A manifest `description`, a README paragraph, and a `.git/description`
//! are attacker-controlled strings, exactly like a file name. This module
//! stores what the source said and normalizes whitespace; it never
//! sanitizes. Sanitizing happens on render — `sanitize_for_terminal` in the
//! text renderer, [`crate::render::html::Html::escape`] in the HTML
//! renderer — never at extraction time, which would corrupt the JSON
//! contract (values there are already made safe by JSON string encoding).
//!
//! ## Parser diagnostics are untrusted content, not messages
//!
//! `toml::de::Error` and `serde_json::Error` both quote the offending input
//! back in their `Display` text (measured in `src/analysis/cargo.rs`'s
//! module doc). Neither is ever put into a warning: a malformed manifest
//! produces a tool-authored constant naming the file, nothing taken from
//! the error value at all.
//!
//! ## Bounded reads (I9)
//!
//! Every file is size-checked with [`std::fs::metadata`] before it is read,
//! via the shared [`super::read_bounded`]: manifests at
//! [`super::cargo::MAX_FILE_BYTES`] (4 MiB), the README at
//! [`README_MAX_BYTES`] (1 MiB), `.git/description` at
//! [`GIT_DESCRIPTION_MAX_BYTES`] (64 KiB). Over the cap, or not valid UTF-8,
//! the source is skipped — warned about only when it is a manifest — rather
//! than read.
//!
//! ## Paths (I8)
//!
//! Every path is a fixed name joined to the scanned root; no path is built
//! from file content. The README lookup is one `read_dir` call, one level,
//! no recursion, and never follows a symlink or descends into a directory.
//!
//! ## Degradation (I10)
//!
//! [`analyze`] never returns an error. A malformed manifest degrades to the
//! next source and adds a [`crate::ScanWarning`] (criterion 6); it does not
//! abort the profile or the scan, because another source may still answer
//! the question. That is why there is no `Failed` variant for this
//! analysis.
//!
//! ## `*.csproj` is deliberately not implemented
//!
//! It needs an XML parser, and none is in this crate's dependency budget
//! for one optional field. Spec 014's Clarifications record the decision.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::cargo::MAX_FILE_BYTES;
use super::{Confidence, read_bounded};
use crate::{Analysis, NotEvaluated, ScanConfig, ScanWarning};

/// A stated purpose for the repository, with the evidence that produced it.
///
/// Every string here is untrusted repository content and must be sanitized
/// by the renderer for its medium, never at extraction time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProjectPurpose {
    /// The stated purpose, whitespace-normalized and capped.
    pub statement: String,
    /// The repository-relative file the statement came from. Criterion 1
    /// makes a finding with no evidence path a defect, so this is not an
    /// `Option`.
    pub evidence: PathBuf,
    /// Which kind of source produced it.
    pub source: PurposeSource,
    /// `Certain` for a manifest field, `Inferred` for prose.
    pub confidence: Confidence,
    /// True when the statement was cut at [`MAX_STATEMENT_CHARS`].
    pub truncated: bool,
}

/// Which kind of source a purpose statement came from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PurposeSource {
    /// The wire zero `Analysis<T>`'s contract requires for a result that did
    /// not run. Never produced by a successful extraction.
    #[default]
    Undetermined,
    /// A package manifest's description field.
    Manifest,
    /// The README's first substantive paragraph.
    Readme,
    /// `.git/description`.
    GitDescription,
}

impl PurposeSource {
    /// The stable snake_case token, matching the JSON contract's spelling.
    pub fn label(self) -> &'static str {
        match self {
            Self::Undetermined => "undetermined",
            Self::Manifest => "manifest",
            Self::Readme => "readme",
            Self::GitDescription => "git_description",
        }
    }
}

/// Version of the purpose precedence table. Bump on any entry or order
/// change.
pub const PURPOSE_TABLE_VERSION: u32 = 1;

/// The cap on a statement's length, in characters — never bytes, so a
/// multi-byte statement is never cut mid-character.
pub const MAX_STATEMENT_CHARS: usize = 500;

/// README candidates larger than this are skipped rather than read.
const README_MAX_BYTES: u64 = 1024 * 1024;

/// `.git/description` larger than this is skipped rather than read.
const GIT_DESCRIPTION_MAX_BYTES: u64 = 64 * 1024;

const CARGO_MANIFEST_FILE: &str = "Cargo.toml";
const PACKAGE_JSON_FILE: &str = "package.json";
const PYPROJECT_TOML_FILE: &str = "pyproject.toml";
const COMPOSER_JSON_FILE: &str = "composer.json";
const GO_MOD_FILE: &str = "go.mod";
const GIT_DESCRIPTION_EVIDENCE: &str = ".git/description";

/// Git's own placeholder text (the start of it) written by `git init`. Not a
/// stated purpose.
const GIT_DESCRIPTION_PLACEHOLDER_PREFIX: &str = "Unnamed repository";

/// The `NotEvaluated` detail when every source in the table missed.
const NO_PURPOSE_FOUND: &str =
    "no manifest description, README paragraph, or Git description found";

/// Tool-authored warning text for a manifest that exists but is oversized,
/// unreadable, or not valid UTF-8. Never anything read from the file itself.
const WARN_UNREADABLE: &str = "could not be read for a purpose statement";

/// Tool-authored warning text for a manifest that exists and was read, but
/// failed to parse. Never anything from the parser's own error value — see
/// the module doc.
const WARN_MALFORMED: &str = "could not be parsed for a purpose statement";

/// README candidate file names, lowercased, in priority order. A tie between
/// two files differing only in case (possible on a case-sensitive
/// filesystem) is broken by the lexicographically smaller actual file name,
/// so the choice never depends on directory order.
const README_CANDIDATES: &[&str] = &[
    "readme.md",
    "readme.markdown",
    "readme.rst",
    "readme.txt",
    "readme",
];

/// Extracts the repository's stated purpose, in the precedence order spec
/// 014 defines.
///
/// Never returns an error. A source that is absent, oversized, non-UTF-8, or
/// malformed is skipped — a malformed manifest additionally producing a
/// warning (criterion 6) — and the next source is tried. When no source
/// yields a statement, the result is `NotEvaluated`, not an empty one
/// (invariant I10).
pub fn analyze(root: &Path, config: &ScanConfig) -> (Analysis<ProjectPurpose>, Vec<ScanWarning>) {
    if !config.read_profile {
        return (Analysis::NotEvaluated(NotEvaluated::Disabled), Vec::new());
    }

    let mut warnings = Vec::new();
    let purpose = find_purpose(root, &mut warnings);

    let analysis = match purpose {
        Some(purpose) => Analysis::Ran(purpose),
        None => Analysis::NotEvaluated(NotEvaluated::InputUnavailable(NO_PURPOSE_FOUND.to_owned())),
    };
    (analysis, warnings)
}

/// Walks the precedence table in order: five manifests, then the README,
/// then `.git/description`. The first non-empty normalized statement wins.
///
/// A manifest that exists but cannot be read or parsed adds a warning to
/// `warnings` and the walk continues (criterion 6); an absent manifest adds
/// nothing, and neither does an absent or unusable README or Git
/// description — only a manifest is common enough, and specific enough
/// about what went wrong, to be worth a warning.
fn find_purpose(root: &Path, warnings: &mut Vec<ScanWarning>) -> Option<ProjectPurpose> {
    if let Some(purpose) = manifest_purpose(root, warnings, CARGO_MANIFEST_FILE, extract_cargo) {
        return Some(purpose);
    }
    if let Some(purpose) =
        manifest_purpose(root, warnings, PACKAGE_JSON_FILE, extract_json_description)
    {
        return Some(purpose);
    }
    if let Some(purpose) = manifest_purpose(root, warnings, PYPROJECT_TOML_FILE, extract_pyproject)
    {
        return Some(purpose);
    }
    if let Some(purpose) =
        manifest_purpose(root, warnings, COMPOSER_JSON_FILE, extract_json_description)
    {
        return Some(purpose);
    }
    if let Some(purpose) = manifest_purpose(root, warnings, GO_MOD_FILE, extract_go_mod) {
        return Some(purpose);
    }
    if let Some(purpose) = readme_purpose(root) {
        return Some(purpose);
    }
    git_description_purpose(root)
}

/// What parsing a manifest's content for a description produced.
enum ManifestExtract {
    /// A string description field was present.
    Found(String),
    /// The manifest parsed, but had no usable description — a miss, not a
    /// failure (spec 014: "a source that exists but states nothing is a
    /// miss, not a stop"). Also covers a non-string value such as
    /// `description.workspace = true`.
    Miss,
    /// The manifest's own content did not parse as its format at all.
    Malformed,
}

/// Reads `file_name` at `root`, bounded at [`MAX_FILE_BYTES`], and runs
/// `extract` over its content.
///
/// A missing file is a silent miss. A file that exists but is oversized,
/// unreadable, or not valid UTF-8 — or that `extract` reports as
/// [`ManifestExtract::Malformed`] — adds a tool-authored warning naming the
/// file and nothing else (see the module doc on parser diagnostics), then
/// misses. Either way the caller moves on to the next source.
fn manifest_purpose(
    root: &Path,
    warnings: &mut Vec<ScanWarning>,
    file_name: &str,
    extract: impl FnOnce(&str) -> ManifestExtract,
) -> Option<ProjectPurpose> {
    let path = root.join(file_name);
    let content = match read_bounded(&path, file_name, MAX_FILE_BYTES) {
        Ok(content) => content,
        Err(NotEvaluated::Failed(_)) => {
            warnings.push(warning(&path, WARN_UNREADABLE));
            return None;
        }
        Err(_) => return None,
    };

    match extract(&content) {
        ManifestExtract::Found(raw) => finish_purpose(
            &raw,
            file_name,
            PurposeSource::Manifest,
            Confidence::Certain,
        ),
        ManifestExtract::Miss => None,
        ManifestExtract::Malformed => {
            warnings.push(warning(&path, WARN_MALFORMED));
            None
        }
    }
}

fn warning(path: &Path, message: &str) -> ScanWarning {
    ScanWarning {
        path: path.to_path_buf(),
        message: message.to_owned(),
    }
}

/// `Cargo.toml`'s `[package].description`. `as_str` returns `None` for a
/// non-string value such as `description.workspace = true`, which is
/// exactly the miss spec 014 asks for — the workspace manifest itself is
/// never read (out of scope for this parcel).
fn extract_cargo(content: &str) -> ManifestExtract {
    match toml::from_str::<toml::Value>(content) {
        Ok(value) => {
            let description = value
                .get("package")
                .and_then(|package| package.get("description"))
                .and_then(toml::Value::as_str);
            match description {
                Some(description) => ManifestExtract::Found(description.to_owned()),
                None => ManifestExtract::Miss,
            }
        }
        Err(_) => ManifestExtract::Malformed,
    }
}

/// `package.json`'s or `composer.json`'s top-level `.description` — the
/// same shape in both formats, so one function serves both entries in the
/// precedence table.
fn extract_json_description(content: &str) -> ManifestExtract {
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(value) => {
            let description = value.get("description").and_then(serde_json::Value::as_str);
            match description {
                Some(description) => ManifestExtract::Found(description.to_owned()),
                None => ManifestExtract::Miss,
            }
        }
        Err(_) => ManifestExtract::Malformed,
    }
}

/// `pyproject.toml`'s `[project].description`, falling back to
/// `[tool.poetry].description`.
fn extract_pyproject(content: &str) -> ManifestExtract {
    match toml::from_str::<toml::Value>(content) {
        Ok(value) => {
            let description = value
                .get("project")
                .and_then(|project| project.get("description"))
                .and_then(toml::Value::as_str)
                .or_else(|| {
                    value
                        .get("tool")
                        .and_then(|tool| tool.get("poetry"))
                        .and_then(|poetry| poetry.get("description"))
                        .and_then(toml::Value::as_str)
                });
            match description {
                Some(description) => ManifestExtract::Found(description.to_owned()),
                None => ManifestExtract::Miss,
            }
        }
        Err(_) => ManifestExtract::Malformed,
    }
}

/// `go.mod`'s `module` directive. Line-oriented and hand-parsed: `go.mod`
/// has no general-purpose parser in this crate's dependency budget, and
/// needs none for one directive.
///
/// Scans at most the first 200 lines for the first whose trimmed form
/// starts with `module` followed by ASCII whitespace; the statement is the
/// remainder, trimmed, with a trailing `// comment` removed. There is no
/// "malformed `go.mod`" case here — a missing or unrecognized directive is
/// simply a miss, never a warning.
fn extract_go_mod(content: &str) -> ManifestExtract {
    for line in content.lines().take(200) {
        let trimmed = line.trim();
        let Some(after_keyword) = trimmed.strip_prefix("module") else {
            continue;
        };
        let Some(rest) = after_keyword.strip_prefix([' ', '\t']) else {
            continue;
        };

        let mut statement = rest.trim();
        if let Some(comment_start) = statement.find("//") {
            statement = statement[..comment_start].trim();
        }

        // `module (` is the valid-but-vanishingly-rare block form; treated
        // as a miss rather than parsed, per the build sheet.
        if statement.is_empty() || statement == "(" {
            continue;
        }

        return ManifestExtract::Found(statement.to_owned());
    }
    ManifestExtract::Miss
}

/// The README path: find the candidate, read it bounded, extract its first
/// substantive paragraph. Any failure along the way — no candidate, an
/// oversized or unreadable file, or no paragraph in it — is a silent miss;
/// only a manifest is specific enough about what went wrong to warrant a
/// warning.
fn readme_purpose(root: &Path) -> Option<ProjectPurpose> {
    let (path, evidence) = find_readme(root)?;
    let content = read_bounded(&path, &evidence, README_MAX_BYTES).ok()?;
    let paragraph = extract_readme_paragraph(&content)?;
    finish_purpose(
        &paragraph,
        &evidence,
        PurposeSource::Readme,
        Confidence::Inferred,
    )
}

/// Chooses the README candidate at `root`, per [`README_CANDIDATES`]'s
/// order, returning its path and its actual on-disk name (the evidence —
/// never the lowercased form used only for matching).
///
/// One `read_dir` call, one level, no recursion. Only plain files are
/// considered: `file_type().is_file()` is false for both a symlink and a
/// directory, which keeps traversal inside the root without following a
/// link (invariant I8).
fn find_readme(root: &Path) -> Option<(PathBuf, String)> {
    let entries = fs::read_dir(root).ok()?;

    let mut best: Option<(usize, String)> = None;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let lower = file_name.to_ascii_lowercase();
        let Some(index) = README_CANDIDATES
            .iter()
            .position(|candidate| *candidate == lower)
        else {
            continue;
        };

        let take = match &best {
            None => true,
            Some((best_index, best_name)) => {
                index < *best_index || (index == *best_index && file_name < best_name.as_str())
            }
        };
        if take {
            best = Some((index, file_name.to_owned()));
        }
    }

    best.map(|(_, name)| (root.join(&name), name))
}

/// `.git/description`, skipped when `.git` is not a directory (a worktree
/// or a submodule, where it is a file) and when the content is empty or
/// Git's own placeholder.
fn git_description_purpose(root: &Path) -> Option<ProjectPurpose> {
    let git_dir = root.join(".git");
    if !git_dir.is_dir() {
        return None;
    }

    let path = git_dir.join("description");
    let content = read_bounded(&path, "description", GIT_DESCRIPTION_MAX_BYTES).ok()?;

    let normalized = normalize_statement(&content);
    if normalized.is_empty() || normalized.starts_with(GIT_DESCRIPTION_PLACEHOLDER_PREFIX) {
        return None;
    }

    finish_purpose(
        &content,
        GIT_DESCRIPTION_EVIDENCE,
        PurposeSource::GitDescription,
        Confidence::Inferred,
    )
}

/// Normalizes `raw` and, if anything remains, caps it — the shared last step
/// for every source in the table, so a TOML multi-line description and a
/// wrapped README paragraph come out the same shape. Returns `None` when
/// the normalized statement is empty, which is a miss like any other.
fn finish_purpose(
    raw: &str,
    evidence: &str,
    source: PurposeSource,
    confidence: Confidence,
) -> Option<ProjectPurpose> {
    let normalized = normalize_statement(raw);
    if normalized.is_empty() {
        return None;
    }
    let (statement, truncated) = cap_statement(&normalized);
    Some(ProjectPurpose {
        statement,
        evidence: PathBuf::from(evidence),
        source,
        confidence,
        truncated,
    })
}

/// Collapses every run of Unicode whitespace to a single space and trims the
/// ends. One shared helper for every source in the table (spec 014's own
/// wording), so a TOML multi-line description and a wrapped README
/// paragraph produce the same shape.
fn normalize_statement(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len());
    let mut in_space_run = false;

    for character in raw.chars() {
        if character.is_whitespace() {
            in_space_run = true;
        } else {
            if in_space_run && !normalized.is_empty() {
                normalized.push(' ');
            }
            in_space_run = false;
            normalized.push(character);
        }
    }

    normalized
}

/// Caps `normalized` at [`MAX_STATEMENT_CHARS`] **characters**, never bytes:
/// a byte slice on a multi-byte boundary panics, which is exactly the
/// invariant-I9 failure this function exists to avoid. When a cut is
/// needed, prefers the last space within the final 80 characters so a word
/// is not split, then trims the result.
fn cap_statement(normalized: &str) -> (String, bool) {
    let char_count = normalized.chars().count();
    if char_count <= MAX_STATEMENT_CHARS {
        return (normalized.to_owned(), false);
    }

    let cut_at = normalized
        .char_indices()
        .nth(MAX_STATEMENT_CHARS)
        .map(|(index, _)| index)
        .unwrap_or(normalized.len());
    let mut slice = &normalized[..cut_at];

    // The byte index 80 characters back from the end of `slice`, found by
    // walking backward over chars rather than bytes, so a multi-byte
    // character is never split.
    let window_start = slice
        .char_indices()
        .rev()
        .nth(79)
        .map(|(index, _)| index)
        .unwrap_or(0);
    if let Some(space_offset) = slice[window_start..].rfind(' ') {
        slice = &slice[..window_start + space_offset];
    }

    (slice.trim_end().to_owned(), true)
}

/// Removes every HTML comment (`<!--` through its matching `-->`, including
/// across line breaks) from `content`. An unterminated `<!--` removes the
/// remainder of the file, matching a real browser's own tolerant behavior.
fn strip_html_comments(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut rest = content;

    loop {
        let Some(start) = rest.find("<!--") else {
            result.push_str(rest);
            break;
        };
        result.push_str(&rest[..start]);

        let after_open = &rest[start + 4..];
        match after_open.find("-->") {
            Some(end) => rest = &after_open[end + 3..],
            None => break,
        }
    }

    result
}

/// Finds the first substantive paragraph in README `content` (spec 014
/// criterion 8), or `None` if the file has no prose at all.
///
/// Comments are stripped first; then lines are walked, skipping empty
/// lines, fenced code (including its closing delimiter), ATX headings,
/// Setext underlines, horizontal rules, HTML blocks, table rows, and
/// badge-only lines, in that order. The first line that is none of those
/// begins the paragraph; consecutive lines are joined with a single space
/// until one is empty, an ATX heading, a fence delimiter, or a table row.
fn extract_readme_paragraph(content: &str) -> Option<String> {
    let stripped = strip_html_comments(strip_front_matter(content));
    let mut lines = stripped.lines().peekable();
    let mut in_fence = false;

    // Whether the line just examined is the *text* of a Setext heading —
    // that is, whether the line after it underlines it with `===` or `---`.
    // A Setext heading is two lines, and skipping only the underline would
    // leave its title standing as the first "substantive" line, which is
    // precisely the case criterion 8 says to skip.
    let underlined_next = |lines: &mut std::iter::Peekable<std::str::Lines<'_>>| {
        lines
            .peek()
            .is_some_and(|next| is_setext_underline(next.trim()))
    };

    let first = loop {
        let line = lines.next()?;
        let trimmed = line.trim();

        if in_fence {
            if is_fence_delimiter(trimmed) {
                in_fence = false;
            }
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }
        if is_fence_delimiter(trimmed) {
            in_fence = true;
            continue;
        }
        if trimmed.starts_with('#')
            || is_setext_underline(trimmed)
            || is_horizontal_rule(trimmed)
            || trimmed.starts_with('<')
            || trimmed.starts_with('|')
            || is_badge_only(trimmed)
        {
            continue;
        }
        if underlined_next(&mut lines) {
            // A Setext heading: drop its underline too, so the next pass
            // does not treat it as a separate line.
            lines.next();
            continue;
        }

        break trimmed;
    };

    let mut collected = vec![first];
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || is_fence_delimiter(trimmed)
            || is_setext_underline(trimmed)
            || is_horizontal_rule(trimmed)
            || trimmed.starts_with('|')
        {
            break;
        }
        // The paragraph ends *before* a line that turns out to be the title
        // of a following Setext heading, rather than absorbing it.
        if underlined_next(&mut lines) {
            break;
        }
        collected.push(trimmed);
    }

    Some(collected.join(" "))
}

/// Returns `content` with a leading YAML (`---`) or TOML (`+++`) front
/// matter block removed.
///
/// Front matter is metadata for a static-site generator, not prose, and its
/// first key reads as a plausible sentence (`title: Docs Site`) — exactly
/// the "a guess presented as fact" that spec 014 forbids. The block is only
/// recognized at the very top of the file and only when it *closes*: a
/// README that merely opens with a horizontal rule keeps all of its content,
/// because an unterminated delimiter would otherwise swallow the file.
fn strip_front_matter(content: &str) -> &str {
    let mut rest = content;
    let mut delimiter = None;

    // Skip leading blank lines to find the opening delimiter, if any.
    while let Some((line, after)) = split_line(rest) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            rest = after;
            continue;
        }
        if trimmed == "---" || trimmed == "+++" {
            delimiter = Some(trimmed.to_owned());
            rest = after;
        }
        break;
    }

    let Some(delimiter) = delimiter else {
        return content;
    };

    // Only strip when the block actually closes.
    while let Some((line, after)) = split_line(rest) {
        if line.trim() == delimiter {
            return after;
        }
        rest = after;
    }

    content
}

/// Splits `text` into its first line and the remainder after that line's
/// terminator, or `None` when `text` is empty.
fn split_line(text: &str) -> Option<(&str, &str)> {
    if text.is_empty() {
        return None;
    }
    match text.find('\n') {
        Some(index) => Some((&text[..index], &text[index + 1..])),
        None => Some((text, "")),
    }
}

fn is_fence_delimiter(trimmed: &str) -> bool {
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// A line consisting only of `=` characters, or only of `-` characters, at
/// least two of them — a Setext heading underline.
fn is_setext_underline(trimmed: &str) -> bool {
    trimmed.len() >= 2
        && (trimmed.chars().all(|ch| ch == '=') || trimmed.chars().all(|ch| ch == '-'))
}

/// Three or more of `-`, `*`, or `_` — one repeated symbol, not a mixture —
/// with only whitespace between them.
fn is_horizontal_rule(trimmed: &str) -> bool {
    for symbol in ['-', '*', '_'] {
        let count = trimmed.chars().filter(|&ch| ch == symbol).count();
        let only_symbol_and_whitespace =
            trimmed.chars().all(|ch| ch == symbol || ch.is_whitespace());
        if count >= 3 && only_symbol_and_whitespace {
            return true;
        }
    }
    false
}

/// True when `trimmed`, once every Markdown link and image is stripped from
/// it, contains nothing but whitespace and the characters `()[]!<>|-` — a
/// row of badges and nothing else.
fn is_badge_only(trimmed: &str) -> bool {
    strip_links_and_images(trimmed)
        .chars()
        .all(|ch| ch.is_whitespace() || "()[]!<>|-".contains(ch))
}

/// Strips every Markdown image (`![alt](url)`), inline link (`[text](url)`),
/// and reference link or image (`![alt][ref]`, `[text][ref]`) from `line`.
///
/// A single forward pass, no backtracking (invariant I9): the label search
/// counts bracket depth so a badge's usual nested `[![alt](img-url)](url)`
/// shape resolves in the same pass rather than a second "nested
/// find/replace" one. The first construct that fails to close — an
/// unclosed `](`, an unclosed `[...][`, or no closing `]` at all — stops the
/// scan there and the remainder of the line is copied through unchanged, so
/// a line of a thousand unmatched `[` characters costs one pass, not one
/// attempt per bracket.
fn strip_links_and_images(line: &str) -> String {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;

    while i < len {
        let is_image_marker = bytes[i] == b'!' && i + 1 < len && bytes[i + 1] == b'[';
        let is_link_marker = bytes[i] == b'[';

        if !is_image_marker && !is_link_marker {
            // `line[i..]` is always a valid char boundary here: `i` only
            // ever advances by a full character's length, or lands on one
            // of the ASCII bytes `!`, `[`, `]`, `(`, `)` matched below,
            // which can never be a UTF-8 continuation byte.
            let character = line[i..].chars().next().unwrap_or('\u{fffd}');
            out.push(character);
            i += character.len_utf8();
            continue;
        }

        let marker = i;
        let bracket = if is_image_marker { i + 1 } else { i };

        let mut depth = 1i32;
        let mut cursor = bracket + 1;
        let close = loop {
            if cursor >= len {
                break None;
            }
            match bytes[cursor] {
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        break Some(cursor);
                    }
                }
                _ => {}
            }
            cursor += 1;
        };

        let Some(close) = close else {
            out.push_str(&line[marker..]);
            return out;
        };
        let after_label = close + 1;

        if after_label < len && bytes[after_label] == b'(' {
            match bytes[after_label + 1..]
                .iter()
                .position(|&byte| byte == b')')
            {
                Some(offset) => {
                    i = after_label + 1 + offset + 1;
                    continue;
                }
                None => {
                    // The unclosed `](` case the build sheet names directly.
                    out.push_str(&line[marker..]);
                    return out;
                }
            }
        }
        if after_label < len && bytes[after_label] == b'[' {
            match bytes[after_label + 1..]
                .iter()
                .position(|&byte| byte == b']')
            {
                Some(offset) => {
                    i = after_label + 1 + offset + 1;
                    continue;
                }
                None => {
                    out.push_str(&line[marker..]);
                    return out;
                }
            }
        }

        // `[label]` (or `![label]`) with nothing recognizable after it is
        // not a link or image: keep it as literal text and resume scanning
        // right after the label's own `]`, which is still forward progress.
        out.push_str(&line[marker..after_label]);
        i = after_label;
    }

    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    /// A minimal temporary directory for this module's own unit tests.
    /// `tests/common/mod.rs::Fixture` is for integration tests under
    /// `tests/`; unit tests here need the same shape without that crate
    /// boundary.
    struct TestRoot {
        path: PathBuf,
    }

    impl TestRoot {
        fn new(label: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "repo-radar-profile-{label}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("test root should be created");
            Self { path }
        }

        fn file(&self, relative: &str, contents: &[u8]) -> &Self {
            let file_path = self.path.join(relative);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).expect("parent directory should be created");
            }
            fs::write(&file_path, contents).expect("test file should be written");
            self
        }

        fn dir(&self, relative: &str) -> &Self {
            fs::create_dir_all(self.path.join(relative)).expect("test directory should be created");
            self
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn assert_manifest_purpose(root: &Path, expected_statement: &str, expected_evidence: &str) {
        let (purpose, warnings) = analyze(root, &ScanConfig::default());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        let Analysis::Ran(purpose) = purpose else {
            panic!("expected a manifest purpose for {expected_evidence}, got {purpose:?}");
        };
        assert_eq!(purpose.statement, expected_statement);
        assert_eq!(purpose.evidence, Path::new(expected_evidence));
        assert_eq!(purpose.confidence, Confidence::Certain);
        assert_eq!(purpose.source, PurposeSource::Manifest);
    }

    #[test]
    fn manifest_description_wins_over_readme() {
        let root = TestRoot::new("manifest-wins");
        root.file(
            "Cargo.toml",
            b"[package]\nname = \"fixture\"\ndescription = \"A fixture crate\"\n",
        );
        root.file(
            "README.md",
            b"# Fixture\n\nSome README prose that should not win.\n",
        );

        let (purpose, warnings) = analyze(&root.path, &ScanConfig::default());

        assert!(warnings.is_empty());
        let Analysis::Ran(purpose) = purpose else {
            panic!("expected the manifest description to be found");
        };
        assert_eq!(purpose.statement, "A fixture crate");
        assert_eq!(purpose.evidence, Path::new("Cargo.toml"));
        assert_eq!(purpose.source, PurposeSource::Manifest);
        assert_eq!(purpose.confidence, Confidence::Certain);
        assert!(!purpose.truncated);
    }

    #[test]
    fn manifest_precedence_follows_the_table() {
        let root = TestRoot::new("precedence");
        root.file(
            "package.json",
            br#"{"description": "package json purpose"}"#,
        );
        root.file(
            "pyproject.toml",
            b"[project]\ndescription = \"pyproject purpose\"\n",
        );

        let (purpose, warnings) = analyze(&root.path, &ScanConfig::default());

        assert!(warnings.is_empty());
        let Analysis::Ran(purpose) = purpose else {
            panic!("expected a purpose to be found");
        };
        assert_eq!(purpose.statement, "package json purpose");
        assert_eq!(purpose.evidence, Path::new("package.json"));
    }

    #[test]
    fn reads_a_description_from_every_supported_manifest() {
        let cargo = TestRoot::new("cargo-desc");
        cargo.file(
            "Cargo.toml",
            b"[package]\nname = \"x\"\ndescription = \"cargo desc\"\n",
        );
        assert_manifest_purpose(&cargo.path, "cargo desc", "Cargo.toml");

        let package_json = TestRoot::new("package-json-desc");
        package_json.file("package.json", br#"{"description": "package json desc"}"#);
        assert_manifest_purpose(&package_json.path, "package json desc", "package.json");

        let pyproject_project = TestRoot::new("pyproject-project-desc");
        pyproject_project.file(
            "pyproject.toml",
            b"[project]\ndescription = \"pyproject project desc\"\n",
        );
        assert_manifest_purpose(
            &pyproject_project.path,
            "pyproject project desc",
            "pyproject.toml",
        );

        let pyproject_poetry = TestRoot::new("pyproject-poetry-desc");
        pyproject_poetry.file(
            "pyproject.toml",
            b"[tool.poetry]\ndescription = \"pyproject poetry desc\"\n",
        );
        assert_manifest_purpose(
            &pyproject_poetry.path,
            "pyproject poetry desc",
            "pyproject.toml",
        );

        let composer = TestRoot::new("composer-desc");
        composer.file("composer.json", br#"{"description": "composer desc"}"#);
        assert_manifest_purpose(&composer.path, "composer desc", "composer.json");

        let go = TestRoot::new("go-mod-desc");
        go.file("go.mod", b"module github.com/example/fixture\n\ngo 1.22\n");
        assert_manifest_purpose(&go.path, "github.com/example/fixture", "go.mod");
    }

    #[test]
    fn a_manifest_without_a_description_falls_through() {
        let root = TestRoot::new("no-description");
        root.file("Cargo.toml", b"[package]\nname = \"fixture\"\n");
        root.file(
            "README.md",
            b"# Fixture\n\nA fixture crate used to exercise the scan engine.\n",
        );

        let (purpose, warnings) = analyze(&root.path, &ScanConfig::default());

        assert!(warnings.is_empty());
        let Analysis::Ran(purpose) = purpose else {
            panic!("expected the README to be used");
        };
        assert_eq!(purpose.source, PurposeSource::Readme);
        assert_eq!(
            purpose.statement,
            "A fixture crate used to exercise the scan engine."
        );
    }

    #[test]
    fn a_workspace_inherited_description_falls_through() {
        let root = TestRoot::new("workspace-inherited");
        root.file(
            "Cargo.toml",
            b"[package]\nname = \"fixture\"\ndescription.workspace = true\n",
        );
        root.file(
            "README.md",
            b"# Fixture\n\nInherited description should not crash the reader.\n",
        );

        let (purpose, warnings) = analyze(&root.path, &ScanConfig::default());

        assert!(
            warnings.is_empty(),
            "a workspace-inherited field is a miss, not a malformed manifest: {warnings:?}"
        );
        let Analysis::Ran(purpose) = purpose else {
            panic!("expected the README to be used");
        };
        assert_eq!(purpose.source, PurposeSource::Readme);
    }

    #[test]
    fn readme_paragraph_skips_comments_headings_and_badges() {
        let readme = "\
<!-- generated -->
# Project Title
=============
[![Build](https://example.invalid/badge.svg)](https://example.invalid/build)
[![Coverage](https://example.invalid/cov.svg)](https://example.invalid/cov)
[![License](https://example.invalid/lic.svg)](https://example.invalid/license)
---

This project does the thing.
";

        let statement = extract_readme_paragraph(readme).expect("real prose follows the noise");

        assert_eq!(statement, "This project does the thing.");
    }

    /// A Setext heading is two lines — its title, then an `===` or `---`
    /// underline — and skipping only the underline leaves the title standing
    /// as the first substantive line. The 6a build sheet's skip rules named
    /// the underline alone, and its own test 6 used an ATX heading, so the
    /// gap survived a green bar and was caught by a live run instead. Both
    /// underline styles are covered here.
    #[test]
    fn readme_paragraph_skips_setext_headings_not_just_their_underlines() {
        let equals = "\
Project Title
=============

The real purpose statement.
";
        let dashes = "\
Project Title
-------------

The real purpose statement.
";

        for readme in [equals, dashes] {
            let statement =
                extract_readme_paragraph(readme).expect("prose follows the Setext heading");

            assert_eq!(
                statement, "The real purpose statement.",
                "a Setext heading's title must not become the purpose"
            );
        }
    }

    /// The mirror case: a paragraph must end *before* the title of a Setext
    /// heading that follows it, rather than absorbing the title and its
    /// underline.
    #[test]
    fn readme_paragraph_stops_before_a_following_setext_heading() {
        let readme = "\
The real purpose statement.
Installation
============
";

        let statement = extract_readme_paragraph(readme).expect("the first paragraph is prose");

        assert_eq!(statement, "The real purpose statement.");
    }

    /// Front matter is a static-site generator's metadata, and its first key
    /// (`title: Docs Site`) reads as a plausible purpose while being nothing
    /// of the kind. Unplanned by the 6a build sheet and found by the same
    /// live run that caught the Setext gap.
    #[test]
    fn readme_paragraph_skips_yaml_and_toml_front_matter() {
        let yaml = "\
---
title: Docs Site
layout: home
---

The actual description.
";
        let toml_matter = "\
+++
title = \"Docs Site\"
+++

The actual description.
";

        for readme in [yaml, toml_matter] {
            let statement = extract_readme_paragraph(readme).expect("prose follows front matter");

            assert_eq!(statement, "The actual description.");
        }
    }

    /// An unterminated delimiter must not swallow the file: a README that
    /// merely opens with a horizontal rule keeps its content.
    #[test]
    fn an_unclosed_front_matter_delimiter_keeps_the_content() {
        let readme = "\
---

The actual description.
";

        let statement = extract_readme_paragraph(readme).expect("content survives");

        assert_eq!(statement, "The actual description.");
    }

    #[test]
    fn readme_paragraph_skips_fenced_code_and_html_blocks() {
        let readme = "\
```rust
fn main() {}
```
<p>Raw HTML block, not the paragraph.</p>

The real purpose statement lives here.
";

        let statement = extract_readme_paragraph(readme).expect("prose follows the noise");

        assert_eq!(statement, "The real purpose statement lives here.");
    }

    #[test]
    fn readme_paragraph_joins_wrapped_lines_and_collapses_whitespace() {
        let readme = "This   is\na   wrapped     paragraph\nspanning three lines.\n";

        let extracted = extract_readme_paragraph(readme).expect("a paragraph is present");
        let normalized = normalize_statement(&extracted);

        assert_eq!(
            normalized,
            "This is a wrapped paragraph spanning three lines."
        );
    }

    #[test]
    fn statement_is_capped_on_a_character_boundary() {
        let paragraph: String = std::iter::repeat_n('日', 2000).collect();
        let readme = format!("{paragraph}\n");

        let extracted = extract_readme_paragraph(&readme).expect("a paragraph is present");
        let normalized = normalize_statement(&extracted);
        let (statement, truncated) = cap_statement(&normalized);

        assert!(statement.chars().count() <= MAX_STATEMENT_CHARS);
        assert!(truncated);
        // A byte-slice implementation panics on a multi-byte boundary before
        // ever producing a `String`; getting this far against a string built
        // entirely of 3-byte characters is the point of the test.
        assert!(std::str::from_utf8(statement.as_bytes()).is_ok());
    }

    #[test]
    fn a_short_statement_is_not_marked_truncated() {
        let (statement, truncated) = cap_statement("A short purpose statement.");

        assert_eq!(statement, "A short purpose statement.");
        assert!(!truncated);
    }

    #[test]
    fn a_readme_with_no_prose_falls_through() {
        let root = TestRoot::new("readme-no-prose");
        root.file(
            "README.md",
            b"# Title\n\n[![Badge](https://example.invalid/b.svg)](https://example.invalid)\n",
        );

        let (purpose, warnings) = analyze(&root.path, &ScanConfig::default());

        assert!(warnings.is_empty());
        assert_eq!(
            purpose,
            Analysis::NotEvaluated(NotEvaluated::InputUnavailable(NO_PURPOSE_FOUND.to_owned()))
        );
    }

    #[test]
    fn readme_candidate_order_is_deterministic() {
        let root = TestRoot::new("readme-order");
        root.file("README.md", b"A markdown purpose statement.\n");
        root.file("README.txt", b"A text purpose statement that must lose.\n");

        let (purpose, warnings) = analyze(&root.path, &ScanConfig::default());

        assert!(warnings.is_empty());
        let Analysis::Ran(purpose) = purpose else {
            panic!("expected a README to be found");
        };
        assert_eq!(
            purpose.evidence,
            Path::new("README.md"),
            "README.md must win over README.txt, deterministically"
        );
        assert_eq!(purpose.statement, "A markdown purpose statement.");
    }

    #[test]
    fn git_description_placeholder_is_ignored() {
        let root = TestRoot::new("git-placeholder");
        root.dir(".git");
        root.file(
            ".git/description",
            b"Unnamed repository; edit this file 'description' to name the repository.\n",
        );

        let (purpose, warnings) = analyze(&root.path, &ScanConfig::default());

        assert!(warnings.is_empty());
        assert_eq!(
            purpose,
            Analysis::NotEvaluated(NotEvaluated::InputUnavailable(NO_PURPOSE_FOUND.to_owned()))
        );
    }

    #[test]
    fn git_description_is_used_and_is_inferred() {
        let root = TestRoot::new("git-description");
        root.dir(".git");
        root.file(".git/description", b"A real, human-written description.\n");

        let (purpose, warnings) = analyze(&root.path, &ScanConfig::default());

        assert!(warnings.is_empty());
        let Analysis::Ran(purpose) = purpose else {
            panic!("expected the git description to be used");
        };
        assert_eq!(purpose.source, PurposeSource::GitDescription);
        assert_eq!(purpose.confidence, Confidence::Inferred);
        assert_eq!(purpose.evidence, Path::new(".git/description"));
        assert_eq!(purpose.statement, "A real, human-written description.");
    }

    #[test]
    fn no_source_reports_not_evaluated_with_a_reason() {
        let root = TestRoot::new("empty");

        let (purpose, warnings) = analyze(&root.path, &ScanConfig::default());

        assert!(warnings.is_empty());
        match purpose {
            Analysis::NotEvaluated(NotEvaluated::InputUnavailable(detail)) => {
                assert!(detail.contains("manifest"));
                assert!(detail.contains("README"));
                assert!(detail.contains("Git"));
            }
            other => panic!("expected NotEvaluated(InputUnavailable(..)), got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_manifest_warns_and_falls_through() {
        let root = TestRoot::new("malformed-manifest");
        // Verbatim from `tests/common/mod.rs::cargo_fixture_malformed`, so
        // the leak channel under test is the one parcel 4c measured.
        root.file(
            "Cargo.toml",
            b"[package]\nname = \"ok\"\nevil = \x1b[31mAPI_KEY_sk_live_abc123 unterminated\n",
        );
        root.file(
            "README.md",
            b"# Fixture\n\nA real purpose statement from the README.\n",
        );

        let (purpose, warnings) = analyze(&root.path, &ScanConfig::default());

        let Analysis::Ran(purpose) = purpose else {
            panic!("the README must still be used despite the malformed manifest");
        };
        assert_eq!(purpose.source, PurposeSource::Readme);

        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].path, root.path.join("Cargo.toml"));
        for forbidden in ["API_KEY", "sk_live", "\u{1b}"] {
            assert!(
                !warnings[0].message.contains(forbidden),
                "warning leaked repository content ({forbidden:?}): {:?}",
                warnings[0].message
            );
        }
    }

    #[test]
    fn disabled_analysis_reports_not_evaluated_disabled() {
        let root = TestRoot::new("disabled");
        root.file(
            "Cargo.toml",
            b"[package]\nname = \"x\"\ndescription = \"should not be read\"\n",
        );

        let config = ScanConfig {
            read_profile: false,
            ..ScanConfig::default()
        };
        let (purpose, warnings) = analyze(&root.path, &config);

        assert_eq!(purpose, Analysis::NotEvaluated(NotEvaluated::Disabled));
        assert!(warnings.is_empty());
    }
}
