//! The human-readable summary printed by default.

use std::fmt;
use std::path::Path;

use crate::{ScanReport, display_path, sanitize_for_terminal};

use super::format_bytes;

/// Writes the default human-readable summary: repository totals, languages,
/// extensions, and the largest files and directories.
///
/// Line counting is an [`crate::Analysis`]: when it did not run, the `Lines:`
/// row and the per-language lines column both say so instead of printing a
/// zero (invariant I10).
pub fn write_summary(out: &mut impl fmt::Write, root: &Path, report: &ScanReport) -> fmt::Result {
    let line_counts = report.lines.ran();

    writeln!(out, "Repository: {}", display_path(root))?;
    writeln!(out, "Files:      {}", report.files)?;
    writeln!(out, "Size:       {}", format_bytes(report.bytes))?;
    if let Some(line_counts) = line_counts {
        writeln!(out, "Lines:      {}", line_counts.lines)?;
    } else {
        writeln!(out, "Lines:      not evaluated")?;
    }

    writeln!(out, "\nLanguages:")?;
    for language in &report.by_language {
        if line_counts.is_some() {
            writeln!(
                out,
                "  {:<16} {:>3} files {:>10}  {:>10} lines",
                sanitize_for_terminal(&language.language),
                language.files,
                format_bytes(language.bytes),
                language.lines
            )?;
        } else {
            writeln!(
                out,
                "  {:<16} {:>3} files {:>10}",
                sanitize_for_terminal(&language.language),
                language.files,
                format_bytes(language.bytes)
            )?;
        }
    }

    writeln!(out, "\nExtensions:")?;
    for (extension, count) in &report.by_extension {
        writeln!(out, "  {:<16} {count}", sanitize_for_terminal(extension))?;
    }

    writeln!(out, "\nLargest files:")?;
    for file in &report.largest_files {
        writeln!(
            out,
            "  {:>10}  {}",
            format_bytes(file.bytes),
            display_path(&file.path)
        )?;
    }

    writeln!(
        out,
        "\nLargest directories (aggregate, including subdirectories):"
    )?;
    for directory in &report.largest_directories {
        writeln!(
            out,
            "  {:>10}  {}",
            format_bytes(directory.bytes),
            display_path(&directory.path)
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::{Analysis, FileEntry, LanguageStat, LineCounts, NotEvaluated};

    #[test]
    fn text_summary_matches_expected_shape() {
        let report = ScanReport {
            files: 1,
            bytes: 10,
            by_language: vec![LanguageStat {
                language: "Rust".to_owned(),
                files: 1,
                bytes: 10,
                lines: 1,
            }],
            largest_files: vec![FileEntry {
                path: std::path::PathBuf::from("src/lib.rs"),
                bytes: 10,
            }],
            lines: Analysis::Ran(LineCounts {
                lines: 1,
                text_files: 1,
                binary_files: 0,
                unreadable_files: 0,
            }),
            ..ScanReport::default()
        };

        let mut output = String::new();
        write_summary(&mut output, Path::new("."), &report)
            .expect("writing to a String cannot fail");

        for heading in [
            "Repository:",
            "Files:",
            "Size:",
            "Lines:",
            "Languages:",
            "Extensions:",
            "Largest files:",
            "Largest directories",
        ] {
            assert!(output.contains(heading), "missing heading '{heading}'");
        }
        assert!(output.ends_with('\n'), "text summary must end in a newline");
    }

    #[test]
    fn text_summary_says_not_evaluated_when_line_counting_is_off() {
        let report = ScanReport {
            files: 1,
            bytes: 10,
            by_language: vec![LanguageStat {
                language: "Rust".to_owned(),
                files: 1,
                bytes: 10,
                lines: 0,
            }],
            lines: Analysis::NotEvaluated(NotEvaluated::Disabled),
            ..ScanReport::default()
        };

        let mut output = String::new();
        write_summary(&mut output, Path::new("."), &report)
            .expect("writing to a String cannot fail");

        assert!(
            output.contains("Lines:      not evaluated"),
            "the Lines row must say not evaluated rather than print a zero"
        );
        for line in output.lines() {
            if line.contains("Rust") {
                assert!(
                    !line.contains("lines"),
                    "a language row must not carry a lines column when line counting is disabled: {line:?}"
                );
            }
        }
    }
}
