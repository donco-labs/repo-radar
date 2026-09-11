//! The versioned JSON contract.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::Serialize;

use crate::{
    Analysis, DirectoryEntry, FileEntry, LanguageStat, LineCounts, ScanReport, ScanWarning,
};

#[derive(Serialize)]
struct JsonReport<'a> {
    version: u8,
    repository: &'a Path,
    files: usize,
    bytes: u64,
    by_extension: &'a BTreeMap<String, usize>,
    language_table_version: u32,
    by_language: &'a [LanguageStat],
    largest_files: &'a [FileEntry],
    largest_directories: &'a [DirectoryEntry],
    lines: &'a Analysis<LineCounts>,
    warnings: &'a [ScanWarning],
}

/// Writes the versioned JSON document: one object, to `out`, with a trailing
/// newline. This is `docs/specs/002-structured-output.md`'s wire contract —
/// every field here is part of that contract and additions must stay
/// additive within schema version 1.
pub fn write_json(out: &mut impl fmt::Write, root: &Path, report: &ScanReport) -> fmt::Result {
    let output = JsonReport {
        version: 1,
        repository: root,
        files: report.files,
        bytes: report.bytes,
        by_extension: &report.by_extension,
        language_table_version: crate::LANGUAGE_TABLE_VERSION,
        by_language: &report.by_language,
        largest_files: &report.largest_files,
        largest_directories: &report.largest_directories,
        lines: &report.lines,
        warnings: &report.warnings,
    };
    // Every field is an owned, string-keyed, float-free value, so the only way
    // serde_json fails here is a bug in a `Serialize` impl in this crate. That is
    // the "invariant the type system cannot express" case in ENGINEERING.md's
    // panic policy, not an error a caller could handle.
    #[allow(clippy::expect_used)]
    let serialized = serde_json::to_string(&output).expect("JSON report should serialize");
    writeln!(out, "{serialized}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use serde_json::Value;

    #[test]
    fn json_render_is_valid_and_versioned() {
        let report = ScanReport::default();

        let mut output = String::new();
        write_json(&mut output, Path::new("."), &report).expect("writing to a String cannot fail");

        let value: Value = serde_json::from_str(output.trim_end()).expect("output should be JSON");
        assert_eq!(value["version"], 1);
        for field in [
            "repository",
            "files",
            "bytes",
            "by_extension",
            "language_table_version",
            "by_language",
            "largest_files",
            "largest_directories",
            "lines",
            "warnings",
        ] {
            assert!(value.get(field).is_some(), "missing field '{field}'");
        }
    }
}
