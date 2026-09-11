//! Git status counts and recent commit activity.
//!
//! This is the first code in the tree that runs a subprocess, and running a
//! subprocess against an untrusted repository is the riskiest thing Repo
//! Radar does. Every invocation goes through [`command`], which carries the
//! full hardening spec 000 requires and one measured, verified exploit
//! blocks: `--no-optional-locks` (invariant I2), `-c core.fsmonitor=false`
//! (invariant I3 — a hostile repository's own `.git/config` can point
//! `core.fsmonitor` at an arbitrary command, which a plain `git status`
//! would otherwise execute), an isolated environment, and no stdin.
//!
//! Only exit codes and porcelain stdout are ever inspected. Git's stderr is
//! never parsed and never reaches the report: it is locale-dependent and a
//! hostile repository can influence it through branch and ref names
//! (invariant I4). Every failure degrades to [`crate::NotEvaluated`] with a
//! tool-authored detail string — this module never panics, and [`analyze`]
//! never returns an error, so a non-Git directory still produces a full
//! report (spec 003, acceptance criterion 4).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::{Analysis, NotEvaluated, ScanConfig};

/// Counts of paths in each Git status category.
///
/// Counts, not paths: this parcel reports how much is in each state and
/// never interprets a file name, which is why it needs no sanitizing. A
/// later parcel that reports paths must run them through
/// `sanitize_for_terminal` first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GitStatus {
    /// Paths with worktree changes not yet staged.
    pub modified: usize,
    /// Paths with staged changes.
    pub staged: usize,
    /// Paths Git does not track and that no ignore rule covers.
    pub untracked: usize,
    /// Paths excluded by an ignore rule.
    pub ignored: usize,
}

/// Commit counts per day over a bounded recent window.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GitActivity {
    /// The window this covers, in days back from today.
    pub window_days: u32,
    /// Total commits in the window.
    pub commits: usize,
    /// Commits per day, ascending by date, omitting days with no commits.
    /// Dates are ISO `YYYY-MM-DD`, from `git log --date=short`.
    pub by_day: Vec<DayCount>,
}

/// One day's commit count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DayCount {
    /// ISO `YYYY-MM-DD`.
    pub date: String,
    /// Commits authored on that date.
    pub commits: usize,
}

// `NotEvaluated` detail strings. These are tool-authored constants, never
// git's own stderr text — stderr is locale-dependent and repository content
// can influence it through ref names, which would put untrusted text into
// the report (invariant I4).
const GIT_UNAVAILABLE: &str = "git is not available";
const NOT_A_WORKTREE: &str = "not a Git worktree";
const NO_COMMITS: &str = "repository has no commits yet";
const STATUS_FAILED: &str = "git status failed";
const STATUS_TOO_LARGE: &str = "status output exceeded the size limit";
const LOG_FAILED: &str = "git log failed";
const HEAD_CHECK_FAILED: &str = "could not check for commits";
const INVALID_UTF8: &str = "git output was not valid UTF-8";

/// Status output larger than this is refused rather than parsed, so a
/// pathological repository cannot turn a truncated count into a wrong one
/// presented as a right one (invariant I9 bounds the input; invariant I10 is
/// why a truncated prefix is refused rather than counted).
const STATUS_SIZE_LIMIT: usize = 8 * 1024 * 1024;

/// Builds a `git` invocation with every hardening measure this module
/// requires. Every call into `git` goes through this constructor so a later
/// edit cannot add an invocation that quietly lacks one.
///
/// - `root` is passed through [`Command::current_dir`], never as an argv
///   element, so it can never be misread as a flag (spec 003, AC 3).
/// - `--no-optional-locks` upholds invariant I2: a read must not refresh the
///   index or take a lock as a side effect.
/// - `-c core.fsmonitor=false` blocks a verified exploit: a hostile
///   repository's own `.git/config` can set `core.fsmonitor` to an
///   arbitrary command that `git status` then executes. That is invariant I3
///   broken by the tool itself. This flag on the command line overrides the
///   repository's config, and it goes on every invocation — including `log`
///   and `rev-parse` — so a later edit that adds a fourth command cannot
///   quietly omit it.
/// - The environment isolates the invocation from the user's own Git
///   configuration and locale, and blocks any prompt; stdin is closed so
///   nothing can prompt it either way.
fn command(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .arg("--no-optional-locks")
        .args(["-c", "core.fsmonitor=false"])
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    command
}

/// Runs both Git analyses against `root`.
///
/// Never returns an error: every failure — no `git` binary, not a worktree,
/// a malformed or oversized response — becomes a `NotEvaluated` reason, so a
/// non-Git directory still produces a successful report (spec 003, AC 4).
pub fn analyze(root: &Path, config: &ScanConfig) -> (Analysis<GitStatus>, Analysis<GitActivity>) {
    if !config.read_git {
        return (
            Analysis::NotEvaluated(NotEvaluated::Disabled),
            Analysis::NotEvaluated(NotEvaluated::Disabled),
        );
    }

    match worktree_status(root) {
        WorktreeStatus::GitUnavailable => (
            not_evaluated_pair(NotEvaluated::InputUnavailable(GIT_UNAVAILABLE.to_owned())),
            not_evaluated_pair(NotEvaluated::InputUnavailable(GIT_UNAVAILABLE.to_owned())),
        ),
        WorktreeStatus::NotAWorktree => (
            not_evaluated_pair(NotEvaluated::InputUnavailable(NOT_A_WORKTREE.to_owned())),
            not_evaluated_pair(NotEvaluated::InputUnavailable(NOT_A_WORKTREE.to_owned())),
        ),
        WorktreeStatus::Worktree => {
            let status = status_analysis(root);
            let activity = match commit_check(root) {
                CommitCheck::HasCommits => activity_analysis(root, config.activity_window_days),
                CommitCheck::NoCommits => {
                    Analysis::NotEvaluated(NotEvaluated::InputUnavailable(NO_COMMITS.to_owned()))
                }
                CommitCheck::CheckFailed => {
                    Analysis::NotEvaluated(NotEvaluated::Failed(HEAD_CHECK_FAILED.to_owned()))
                }
            };
            (status, activity)
        }
    }
}

/// A `NotEvaluated` reason applied identically to both result types, since
/// the two share every failure mode up to and including whether `root` is a
/// worktree at all.
fn not_evaluated_pair<T>(reason: NotEvaluated) -> Analysis<T> {
    Analysis::NotEvaluated(reason)
}

/// What `rev-parse --is-inside-work-tree` established about `root`.
enum WorktreeStatus {
    /// `git` could not be spawned at all, e.g. not on `PATH`.
    GitUnavailable,
    /// `git` ran, but `root` is not inside a worktree — including a bare
    /// repository, which exits 0 and prints `false` rather than failing.
    NotAWorktree,
    /// `root` is inside a Git worktree.
    Worktree,
}

fn worktree_status(root: &Path) -> WorktreeStatus {
    let output = match command(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return WorktreeStatus::GitUnavailable,
    };

    if !output.status.success() {
        return WorktreeStatus::NotAWorktree;
    }

    // A bare repository exits 0 here and prints `false`; the exit code alone
    // is not enough, so the stdout text is checked too.
    match String::from_utf8(output.stdout) {
        Ok(stdout) if stdout.trim() == "true" => WorktreeStatus::Worktree,
        _ => WorktreeStatus::NotAWorktree,
    }
}

/// What `rev-parse --verify HEAD` established.
enum CommitCheck {
    HasCommits,
    /// The repository has no commits yet. `git log` itself reports this by
    /// exiting 128 with a localized message; checking `HEAD` first avoids
    /// ever parsing that message.
    NoCommits,
    /// `git` could not be run for this check, despite having just run
    /// successfully for the worktree check above.
    CheckFailed,
}

fn commit_check(root: &Path) -> CommitCheck {
    match command(root)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
    {
        Ok(output) if output.status.success() => CommitCheck::HasCommits,
        Ok(_) => CommitCheck::NoCommits,
        Err(_) => CommitCheck::CheckFailed,
    }
}

fn status_analysis(root: &Path) -> Analysis<GitStatus> {
    let output = match command(root)
        .args(["status", "--porcelain=v1", "--ignored=matching"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return Analysis::NotEvaluated(NotEvaluated::Failed(STATUS_FAILED.to_owned())),
    };

    if !output.status.success() {
        return Analysis::NotEvaluated(NotEvaluated::Failed(STATUS_FAILED.to_owned()));
    }

    // Reject an oversized response outright rather than counting a
    // truncated prefix: a wrong number presented as a right one is the
    // invariant I10 failure this refusal avoids.
    if output.stdout.len() > STATUS_SIZE_LIMIT {
        return Analysis::NotEvaluated(NotEvaluated::Failed(STATUS_TOO_LARGE.to_owned()));
    }

    let stdout = match String::from_utf8(output.stdout) {
        Ok(stdout) => stdout,
        Err(_) => return Analysis::NotEvaluated(NotEvaluated::Failed(INVALID_UTF8.to_owned())),
    };

    Analysis::Ran(parse_porcelain(&stdout))
}

/// Classifies porcelain v1 status lines. Only the first two bytes of each
/// line are read; everything else, including a rename's ` -> ` separator,
/// needs no special handling because it is never inspected.
///
/// A file name containing a newline is C-quoted by Git, so an entry with an
/// embedded `\n` still stays on a single line — parsing by line is safe
/// without a `-z` null separator (measured; see the build sheet).
fn parse_porcelain(stdout: &str) -> GitStatus {
    let mut status = GitStatus::default();

    for line in stdout.lines() {
        let bytes = line.as_bytes();
        if bytes.len() < 3 {
            continue;
        }
        let (x, y) = (bytes[0], bytes[1]);

        match (x, y) {
            (b'?', b'?') => status.untracked += 1,
            (b'!', b'!') => status.ignored += 1,
            _ => {
                // A path can carry staged changes and further unstaged
                // edits at once; the two counts are independent, not a
                // partition of the entries.
                if x != b' ' {
                    status.staged += 1;
                }
                if y != b' ' {
                    status.modified += 1;
                }
            }
        }
    }

    status
}

fn activity_analysis(root: &Path, window_days: u32) -> Analysis<GitActivity> {
    let since = format!("--since={window_days}.days");
    // `tformat:` terminates every record including the last, unlike
    // `format:`, which omits the trailing newline and runs the final date
    // into whatever follows it (measured).
    let output = match command(root)
        .args([
            "log",
            since.as_str(),
            "--date=short",
            "--pretty=tformat:%ad",
            "--max-count=50000",
        ])
        .output()
    {
        Ok(output) => output,
        Err(_) => return Analysis::NotEvaluated(NotEvaluated::Failed(LOG_FAILED.to_owned())),
    };

    if !output.status.success() {
        return Analysis::NotEvaluated(NotEvaluated::Failed(LOG_FAILED.to_owned()));
    }

    let stdout = match String::from_utf8(output.stdout) {
        Ok(stdout) => stdout,
        Err(_) => return Analysis::NotEvaluated(NotEvaluated::Failed(INVALID_UTF8.to_owned())),
    };

    Analysis::Ran(group_by_day(&stdout, window_days))
}

/// Groups `tformat:%ad` dates (one per line, `YYYY-MM-DD`) into ascending
/// per-day counts.
///
/// `BTreeMap` sorts its keys, and lexical order on the ISO `YYYY-MM-DD`
/// format is chronological order — the explicit sort ENGINEERING.md's
/// determinism rule requires, rather than trusting `git log`'s own order.
/// A date with no commits is never inserted, so gaps produce no zero row.
fn group_by_day(stdout: &str, window_days: u32) -> GitActivity {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut commits = 0usize;

    for line in stdout.lines() {
        let date = line.trim();
        if date.is_empty() {
            continue;
        }
        *counts.entry(date).or_insert(0) += 1;
        commits += 1;
    }

    let by_day = counts
        .into_iter()
        .map(|(date, commits)| DayCount {
            date: date.to_owned(),
            commits,
        })
        .collect();

    GitActivity {
        window_days,
        commits,
        by_day,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn porcelain_classifies_each_status_prefix() {
        assert_eq!(
            parse_porcelain(" M a"),
            GitStatus {
                modified: 1,
                ..GitStatus::default()
            }
        );
        assert_eq!(
            parse_porcelain("M  a"),
            GitStatus {
                staged: 1,
                ..GitStatus::default()
            }
        );
        assert_eq!(
            parse_porcelain("MM a"),
            GitStatus {
                staged: 1,
                modified: 1,
                ..GitStatus::default()
            },
            "a path with staged changes and further unstaged edits is genuinely in both states"
        );
        assert_eq!(
            parse_porcelain("?? a"),
            GitStatus {
                untracked: 1,
                ..GitStatus::default()
            }
        );
        assert_eq!(
            parse_porcelain("!! a"),
            GitStatus {
                ignored: 1,
                ..GitStatus::default()
            }
        );
    }

    #[test]
    fn porcelain_ignores_short_and_empty_lines() {
        assert_eq!(parse_porcelain(""), GitStatus::default());
        assert_eq!(parse_porcelain("M"), GitStatus::default());
        assert_eq!(parse_porcelain("??"), GitStatus::default());
    }

    #[test]
    fn porcelain_counts_quoted_paths_as_one_entry() {
        // Git C-quotes a name containing a control character such as `\n`,
        // so the quoted form never breaks across lines.
        let quoted = "?? \"evil\\nnewline.txt\"";

        assert_eq!(
            parse_porcelain(quoted),
            GitStatus {
                untracked: 1,
                ..GitStatus::default()
            }
        );
    }

    #[test]
    fn activity_groups_commits_by_day_ascending() {
        let log = "2026-09-03\n2026-09-01\n2026-09-03\n2026-09-02\n";

        let activity = group_by_day(log, 30);

        assert_eq!(activity.commits, 4);
        assert_eq!(
            activity.by_day,
            vec![
                DayCount {
                    date: "2026-09-01".to_owned(),
                    commits: 1,
                },
                DayCount {
                    date: "2026-09-02".to_owned(),
                    commits: 1,
                },
                DayCount {
                    date: "2026-09-03".to_owned(),
                    commits: 2,
                },
            ],
            "unsorted input must produce ascending by_day with correct per-day counts"
        );
    }

    #[test]
    fn activity_omits_days_with_no_commits() {
        let log = "2026-09-01\n2026-09-05\n";

        let activity = group_by_day(log, 30);

        assert_eq!(
            activity.by_day,
            vec![
                DayCount {
                    date: "2026-09-01".to_owned(),
                    commits: 1,
                },
                DayCount {
                    date: "2026-09-05".to_owned(),
                    commits: 1,
                },
            ],
            "a gap between two dates must not produce a zero row"
        );
    }
}
