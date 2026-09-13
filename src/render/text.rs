//! The human-readable summary printed by default.

use std::fmt;
use std::path::Path;

use crate::{
    Analysis, CargoManifest, NotEvaluated, ScanReport, display_path, sanitize_for_terminal,
};

use super::format_bytes;

/// Writes the default human-readable summary: repository totals, languages,
/// extensions, the largest files and directories, and Git status and
/// activity.
///
/// Line counting and both Git analyses are [`crate::Analysis`]: when one did
/// not run, its row says so — naming the reason for Git, since "not a Git
/// worktree" and "disabled" are different things a user needs told apart —
/// instead of printing a zero (invariant I10).
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

    writeln!(out, "\nGit:")?;
    match &report.git_status {
        Analysis::Ran(status) => writeln!(
            out,
            "  Status:   {} modified, {} staged, {} untracked, {} ignored",
            status.modified, status.staged, status.untracked, status.ignored
        )?,
        Analysis::NotEvaluated(reason) => writeln!(
            out,
            "  Status:   not evaluated ({})",
            not_evaluated_text(reason)
        )?,
    }
    match &report.git_activity {
        Analysis::Ran(activity) => writeln!(
            out,
            "  Activity: {} commits in the last {} days",
            activity.commits, activity.window_days
        )?,
        Analysis::NotEvaluated(reason) => writeln!(
            out,
            "  Activity: not evaluated ({})",
            not_evaluated_text(reason)
        )?,
    }

    writeln!(out, "\nCargo:")?;
    match &report.cargo_manifest {
        Analysis::Ran(manifest) => {
            writeln!(out, "  Package:      {}", manifest_summary(manifest))?;
            writeln!(
                out,
                "  Dependencies: {} direct",
                manifest.dependencies.len()
            )?;
            for dependency in &manifest.dependencies {
                let requirement = dependency
                    .requirement
                    .as_deref()
                    .map(sanitize_for_terminal)
                    .unwrap_or_else(|| "-".to_owned());
                let target = dependency
                    .target
                    .as_deref()
                    .map(sanitize_for_terminal)
                    .unwrap_or_default();
                writeln!(
                    out,
                    "    {:<6} {:<24} {:<10} {:<9} {}",
                    dependency.kind.label(),
                    sanitize_for_terminal(&dependency.name),
                    requirement,
                    dependency.source.label(),
                    target
                )?;
            }
        }
        Analysis::NotEvaluated(reason) => writeln!(
            out,
            "  Manifest:     not evaluated ({})",
            not_evaluated_text(reason)
        )?,
    }
    match &report.cargo_lock {
        Analysis::Ran(lock) => writeln!(
            out,
            "  Lockfile:     {} packages, {} local",
            lock.packages, lock.local_packages
        )?,
        Analysis::NotEvaluated(reason) => writeln!(
            out,
            "  Lockfile:     not evaluated ({})",
            not_evaluated_text(reason)
        )?,
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

/// A short summary of the package a manifest declares: `"name version"` when
/// both are present, just the name when the version is not, or a label for
/// the two cases with no package at all. `name` and `version` are untrusted
/// manifest content and are sanitized before display.
fn manifest_summary(manifest: &CargoManifest) -> String {
    match (&manifest.package_name, &manifest.package_version) {
        (Some(name), Some(version)) => format!(
            "{} {}",
            sanitize_for_terminal(name),
            sanitize_for_terminal(version)
        ),
        (Some(name), None) => sanitize_for_terminal(name),
        (None, _) if manifest.workspace_root => "virtual workspace".to_owned(),
        (None, _) => "unnamed package".to_owned(),
    }
}

/// A short, human-readable reason for a `NotEvaluated` result.
///
/// The detail, when present, is more specific than the wire token
/// (`"not a Git worktree"` rather than `input_unavailable`); every detail
/// string is tool-authored (see `src/analysis/git.rs` and
/// `src/analysis/cargo.rs`), never repository content, so no sanitizing is
/// needed here.
fn not_evaluated_text(reason: &NotEvaluated) -> &str {
    reason.detail().unwrap_or_else(|| reason.reason())
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
            "Git:",
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

    #[test]
    fn text_summary_reports_git_results_when_evaluated() {
        let report = ScanReport {
            git_status: Analysis::Ran(crate::GitStatus {
                modified: 2,
                staged: 1,
                untracked: 3,
                ignored: 4,
            }),
            git_activity: Analysis::Ran(crate::GitActivity {
                window_days: 30,
                commits: 5,
                by_day: Vec::new(),
            }),
            ..ScanReport::default()
        };

        let mut output = String::new();
        write_summary(&mut output, Path::new("."), &report)
            .expect("writing to a String cannot fail");

        assert!(output.contains("2 modified, 1 staged, 3 untracked, 4 ignored"));
        assert!(output.contains("5 commits in the last 30 days"));
    }

    #[test]
    fn text_summary_names_the_reason_when_git_was_not_evaluated() {
        let report = ScanReport {
            git_status: Analysis::NotEvaluated(NotEvaluated::InputUnavailable(
                "not a Git worktree".to_owned(),
            )),
            git_activity: Analysis::NotEvaluated(NotEvaluated::InputUnavailable(
                "not a Git worktree".to_owned(),
            )),
            ..ScanReport::default()
        };

        let mut output = String::new();
        write_summary(&mut output, Path::new("."), &report)
            .expect("writing to a String cannot fail");

        assert!(
            output.contains("Status:   not evaluated (not a Git worktree)"),
            "a not-evaluated Git row must name its reason, not just say not evaluated: {output:?}"
        );
        assert!(output.contains("Activity: not evaluated (not a Git worktree)"));
    }
}
