//! The versioned JSON contract.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::Serialize;

use crate::{DirectoryEntry, FileEntry, LanguageStat, LineCounts, ScanReport, ScanWarning};

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
    lines: &'a LineCounts,
    warnings: &'a [ScanWarning],
}

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
    let serialized = serde_json::to_string(&output).expect("JSON report should serialize");
    writeln!(out, "{serialized}")
}

#[cfg(test)]
mod tests {
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
