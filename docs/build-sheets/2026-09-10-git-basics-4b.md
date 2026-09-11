# Build sheet: Git basics — status counts and recent activity, parcel 4b

Date: 2026-09-10
Spec: [003 repository intelligence](../specs/003-repository-intelligence.md) (acceptance criteria 3, 4, and the 4b row), [000 safety invariants](../specs/000-safety-invariants.md) (I2, I3, I4, I9), [002 structured output](../specs/002-structured-output.md)
Branch: `feat/git-basics`

## Goal

Add the two Git analyses spec 003 assigns to parcel 4b:

1. **Status counts** — how many paths are modified, staged, untracked, and ignored, when the root is a Git worktree.
2. **Recent commit activity** — commits per day over a configurable window.

Both are `Analysis<T>` from the start. This parcel is the first consumer of the seam 5b built, and the first code in the tree that runs a subprocess — which is where nearly all of its risk lives.

**A non-Git directory must still produce a successful base report** (AC 4). Every failure mode degrades to `NotEvaluated`; none of them is an error that aborts the scan.

## Why

Spec 003 wants the report to help someone decide what deserves attention first. "Forty-one modified files and no commits in three weeks" is a different repository from "clean tree, twelve commits today", and neither is visible from file counts.

This parcel also establishes how Repo Radar talks to `git` at all. Specs 013 (provenance), 021 (activity and hotspots), and 023 (forge metadata) all read Git, and every one of them will copy whatever this parcel does. The hardening below is therefore not defensive padding on a small feature — it is the shape three later phases inherit, and getting it wrong here is expensive to undo. This is the same reason 5b landed before 4b.

## Decision: shell out to `git`, no new dependency

Invariant I2 says "read-only plumbing only. Git must be invoked with optional locks disabled" — the invariant document already assumes a `git` subprocess and constrains how it is called. AC 3 likewise says Git analysis "never shells out with user-controlled arguments", which regulates shelling out rather than forbidding it.

Considered and declined, per ENGINEERING.md's requirement that a dependency decision state its alternatives:

- **`git2`** (libgit2 bindings) — a C library, which fights `#![forbid(unsafe_code)]`'s spirit and is a large supply-chain surface on a tool whose pitch is being safe to point at unread code.
- **`gix`** (pure Rust) — genuinely good, and the right answer if this project were optimizing for speed or for avoiding a `git` install. It is a large dependency tree against a deliberately tight budget, for two analyses that are a `status` and a `log` away.
- **Parsing `.git` by hand** — refs and loose objects are tractable; packfiles, the index format, and delta chains are not. Rejected as disproportionate.

**No new dependency is added by this parcel.** If `git` is not on `PATH`, both analyses report `NotEvaluated` and the base report still succeeds.

## Safety design — read this section before writing any code

Running a subprocess against an untrusted repository is the single riskiest thing in the tree. Four properties below were verified empirically against git 2.x on 2026-09-10; the measurements are recorded so a later reader can re-check them rather than trust them.

### 1. The root path never enters the argument vector

Use `Command::current_dir(root)`, **not** `git -C <path>`. The root comes from the command line, and a path such as `--upload-pack=evil` passed as an argv element is a flag, not a path. Keeping it out of argv entirely means there is no quoting question to get wrong.

`Command` passes an argv directly to `execve`; there is no shell, so no interpolation is possible. That is what AC 3 and I4 require.

### 2. `--no-optional-locks` on every invocation

Mandated by I2. Git documents `GIT_OPTIONAL_LOCKS=0` (equivalently `--no-optional-locks`) as preventing `git status` from refreshing the index as a side effect.

**Measured honestly:** attempts to make a local `git status` rewrite `.git/index` — stale stat data, identical content with a new mtime — did **not** reproduce an index write on the tested git version. That does not make the flag optional. I2 requires it, git documents the write as possible, and the immutability harness compares `.git` byte-for-byte including mtimes, so a version that does write would turn into a red bar rather than a silent violation. Pass the flag; do not conclude from the failed repro that it is unnecessary.

### 3. `-c core.fsmonitor=false` — this one is load-bearing, and it is a real exploit

**Verified**: a repository whose own `.git/config` contains `core.fsmonitor = ./evil.sh` executes that script on `git status`.

```
$ git -C <hostile> status --porcelain=v1
PWNED-fsmonitor-executed        <- from the repo's own config
PWNED-fsmonitor-executed
 M a.txt
```

That is **invariant I3 broken** — repository content executed — by a plain `git status` against a clone the user has not read. It is exactly the attack Repo Radar exists to protect against, delivered through the tool itself.

```
$ git -C <hostile> --no-optional-locks -c core.fsmonitor=false status --porcelain=v1
 M a.txt                        <- no execution
```

`-c core.fsmonitor=false` on the command line overrides the repository's config and blocks it. **Every `git` invocation in this parcel carries it**, including `log` and `rev-parse`, so that no later edit can add a status call that quietly lacks it.

Also tested and found **not** to execute during `status --porcelain` on this version: `filter.<name>.clean` via `.gitattributes`, `core.hooksPath` with a `post-index-change` hook, and `core.alternateRefsCommand` during `log`. They are recorded here as checked-and-clear, not as guaranteed-safe forever — a future parcel that adds `git diff`, `git cat-file`, or a checkout-shaped operation re-enters filter-driver territory and must re-test.

### 4. Isolate from ambient configuration

Set on every invocation:

| Setting | Why |
| --- | --- |
| `GIT_CONFIG_SYSTEM=/dev/null` | System config cannot change results or re-enable a blocked feature |
| `GIT_CONFIG_GLOBAL=/dev/null` | The user's own `~/.gitconfig` cannot change what Repo Radar reports; two users scanning one repository get the same answer |
| `GIT_TERMINAL_PROMPT=0` | Git can never block waiting on input |
| `LC_ALL=C` | Deterministic output, and no locale-dependent text |
| `Stdio::null()` on stdin | Nothing can prompt |
| `Stdio::piped()` on stdout/stderr | Nothing reaches the user's terminal unsanitized |

The existing `tests/common/mod.rs::git` helper already sets the two `GIT_CONFIG_*` variables — match it.

### 5. Never parse stderr; never put git's text in the report

Porcelain formats are documented as stable and locale-independent; stderr messages are neither. Decide everything from the **exit status** and **porcelain stdout**.

`NotEvaluated` detail strings are **tool-authored constants** — `"not a Git worktree"`, `"git is not available"`, `"repository has no commits yet"`. Never pass git's stderr through into the report: it is locale-dependent, and a hostile repository can influence it through branch and ref names, which would put untrusted text into the model.

### 6. Bounded output (I9)

`Command::output()` reads all of stdout into memory. A repository with millions of commits or millions of untracked files would be an unbounded allocation.

Cap both:

- `log` takes `--max-count=50000`.
- `status` output is read to a `String` and, if it exceeds **8 MiB**, the analysis reports `NotEvaluated::Failed("status output exceeded the size limit")` rather than counting a truncated prefix. A truncated count is a wrong number presented as a right one, which is the I10 failure.

### 7. Path parsing — measured, not assumed

Porcelain v1 emits one line per entry. A file name containing a newline is C-quoted, so line-based parsing is safe:

```
$ git status --porcelain=v1        # tree contains a file named "evil\nnewline.txt"
?? "back\\slash.txt"
?? "evil\nnewline.txt"
?? "quote\"file.txt"
$ ... | wc -l
3                                  # 3 entries, 3 lines
```

**Checked and found not to matter:** setting `core.quotePath=false` in the repository's own config does **not** break this — git always C-quotes control characters regardless, because the format would otherwise be ambiguous. Line count stayed 3 either way.

So: parse by line, and **do not** add `-c core.quotePath=true` on the strength of a threat that was measured not to exist. This parcel counts entries and never interprets a path, which is the deeper reason it is safe — but if a later parcel starts reporting paths from status output, it must sanitize them through `sanitize_for_terminal` and should revisit `-z`.

## Module placement

Create the `analysis/` module ENGINEERING.md's target tree specifies, with **only** this parcel's code in it:

```text
src/analysis/
  mod.rs      Declares `pub mod git;`. Nothing else yet.
  git.rs      Everything in this parcel.
```

**Do not move existing code.** `Analysis`, `NotEvaluated`, `LineCounts`, and the line-counting and language logic stay in `src/lib.rs`. Relocating them is the deferred refactor parcel's job, and mixing a move with a feature makes both harder to review. New code goes in its target location; old code moves later.

`src/lib.rs` declares `pub mod analysis;` and re-exports the two public result types so `repo_radar::GitStatus` resolves.

## Seam contracts

### `GitStatus` — in `src/analysis/git.rs`

```rust
/// Counts of paths in each Git status category.
///
/// Counts, not paths: this parcel reports how much is in each state and never
/// interprets a file name, which is why it needs no sanitizing. A later parcel
/// that reports paths must run them through `sanitize_for_terminal` first.
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
```

`Default` is required by the `Analysis<T>` `Serialize` impl's `T: Default` bound.

### `GitActivity` — in `src/analysis/git.rs`

```rust
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
```

Days with no commits are **omitted**, not emitted as zero. A consumer that wants a dense series can fill from `window_days`; the model does not invent rows.

`by_day` must be sorted ascending by date explicitly — ENGINEERING.md's determinism rule. Do not rely on `git log`'s ordering.

### `ScanReport` additions — in `src/lib.rs`

```rust
    /// Git status counts, or why they were not produced.
    pub git_status: Analysis<GitStatus>,
    /// Recent commit activity, or why it was not produced.
    pub git_activity: Analysis<GitActivity>,
```

Two fields rather than one `Analysis<GitInfo>`, because the two genuinely differ: an empty repository is a worktree whose status runs fine and whose activity has nothing to report. One combined analysis would have to collapse that into a single reason and lose it.

### `ScanConfig` additions

```rust
    /// Run the Git analyses. Defaults to true.
    pub read_git: bool,
    /// Days back the activity window covers. Defaults to 30.
    pub activity_window_days: u32,
```

### The entry point

```rust
/// Runs both Git analyses against `root`.
///
/// Never returns an error: every failure — no `git` binary, not a worktree,
/// a malformed or oversized response — becomes a `NotEvaluated` reason, so a
/// non-Git directory still produces a successful report (spec 003, AC 4).
pub fn analyze(root: &Path, config: &ScanConfig) -> (Analysis<GitStatus>, Analysis<GitActivity>);
```

## Reason mapping

Every path below is reachable; each has a test.

| Condition | Result |
| --- | --- |
| `read_git` is false | `NotEvaluated(Disabled)` for both |
| `git` not on `PATH` (spawn fails `ErrorKind::NotFound`) | `NotEvaluated(InputUnavailable("git is not available"))` for both |
| `rev-parse --is-inside-work-tree` exits non-zero | `NotEvaluated(InputUnavailable("not a Git worktree"))` for both |
| `rev-parse --is-inside-work-tree` prints `false` (bare repository) | `NotEvaluated(InputUnavailable("not a Git worktree"))` for both |
| Worktree, but `rev-parse --verify HEAD` fails (no commits) | status runs normally; activity is `NotEvaluated(InputUnavailable("repository has no commits yet"))` |
| `status` exits non-zero | status is `NotEvaluated(Failed("git status failed"))`; activity is unaffected |
| `status` stdout exceeds 8 MiB | `NotEvaluated(Failed("status output exceeded the size limit"))` |
| stdout is not valid UTF-8 | `NotEvaluated(Failed("git output was not valid UTF-8"))` |
| Otherwise | `Ran(..)` |

**The empty-repository case is why `rev-parse --verify HEAD` exists.** Measured: `git log` in a repository with no commits exits **128** with `fatal: your current branch 'main' does not have any commits yet`. Detecting that by matching the message would be parsing localized stderr, which section 5 forbids. Checking `HEAD` first is a clean exit-code test that says the same thing.

Measured exit codes: non-Git directory → `rev-parse` exits **128**. Bare repository → `rev-parse --is-inside-work-tree` exits **0** and prints `false`, so the exit code alone is not enough; **check the stdout text too.**

## The commands

All three carry the full hardening from the safety section.

```rust
// Shared construction — every invocation goes through this.
fn command(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(root)                        // root never enters argv
        .arg("--no-optional-locks")               // I2
        .args(["-c", "core.fsmonitor=false"])     // I3 — blocks config-driven execution
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    command
}
```

| Purpose | Arguments |
| --- | --- |
| Worktree detection | `rev-parse --is-inside-work-tree` |
| Commit presence | `rev-parse --verify HEAD` |
| Status | `status --porcelain=v1 --ignored=matching` |
| Activity | `log --since=<N>.days --date=short --pretty=tformat:%ad --max-count=50000` |

`<N>` is `config.activity_window_days`, a `u32`, formatted with `format!("--since={n}.days")`. It is a number the CLI parsed into an integer, never a user string passed through.

Use `tformat:` and not `format:`. **Measured:** `--pretty=format:%ad` omits the trailing newline, so the last date runs into whatever follows it; `tformat:` terminates every record including the last.

`--ignored=matching` reports ignored directories as one entry (`!! ig/`) rather than expanding them, which keeps output bounded on a repository with a large ignored `target/`.

## Parsing porcelain v1

Each line is `XY<space><path>`. Classify on the first two bytes only:

| Prefix | Counts as |
| --- | --- |
| `??` | `untracked` |
| `!!` | `ignored` |
| otherwise | `X != ' '` → `staged`; `Y != ' '` → `modified` |

A line can increment both `staged` and `modified` — a path with staged changes and further unstaged edits is genuinely in both states. The counts are per-category, not a partition, and the field docs must say so.

Skip lines shorter than 3 bytes rather than indexing into them. Rename entries contain ` -> ` in the path portion; since only the prefix is read, they need no special handling.

Verified fixture output:

```
 M a.txt        -> modified
?? b.txt        -> untracked
!! ig/          -> ignored
```

## Files

| File | Change |
| --- | --- |
| `src/analysis/mod.rs` | New. `//!` docs, `pub mod git;`. |
| `src/analysis/git.rs` | New. Everything above: `GitStatus`, `GitActivity`, `DayCount`, `command`, `analyze`, parsing, unit tests. |
| `src/lib.rs` | `pub mod analysis;`. Re-export `GitStatus`, `GitActivity`, `DayCount`. Two `ScanReport` fields, two `ScanConfig` fields, `Default` for both. Call `analysis::git::analyze` in `scan`. |
| `src/render/text.rs` | A `Git:` block for both analyses, each stating `not evaluated` with its reason when it did not run. |
| `src/render/json.rs` | Two `JsonReport` fields. Additive within schema version 1. |
| `src/render/html/mod.rs` | A Git section, same `NotEvaluated` handling. |
| `src/main.rs` | `--no-git` and `--since-days <N>`. Help text. |
| `tests/common/mod.rs` | Extend the fixture helpers — see Tests. |
| `tests/safety_invariants.rs` | New Git-specific invariant tests — see Tests. |
| `tests/structured_output.rs` | Assert the new JSON fields for ran and not-evaluated. |
| `docs/specs/003-repository-intelligence.md` | Mark 4b delivered; record the `core.fsmonitor` finding under Clarifications. |
| `docs/ROADMAP.md` | Phase 4 stays `In progress` — 4c is still outstanding. |
| `README.md` | Document the Git fields and both flags. |

## Tests

### Unit, in `src/analysis/git.rs`

Parsing is pure and gets tested directly, with no subprocess:

1. `porcelain_classifies_each_status_prefix` — `" M a"`, `"M  a"`, `"MM a"`, `"?? a"`, `"!! a"` produce the documented counts; `"MM a"` increments both `staged` and `modified`.
2. `porcelain_ignores_short_and_empty_lines` — `""`, `"M"`, `"??"` are skipped without panicking.
3. `porcelain_counts_quoted_paths_as_one_entry` — a C-quoted name containing `\n` counts as exactly one untracked entry.
4. `activity_groups_commits_by_day_ascending` — unsorted `YYYY-MM-DD` input produces ascending `by_day` with correct per-day counts and a correct total.
5. `activity_omits_days_with_no_commits` — a gap between two dates produces no zero row.

### Integration, in `tests/safety_invariants.rs`

All inside `assert_target_unchanged`, per ENGINEERING.md's non-negotiable rule.

6. `i2_git_analysis_does_not_mutate_git_directory` — extend the existing I2 test to cover a run with the Git analyses **enabled** against a fixture whose index has been made stale (rewrite a tracked file with identical content and a newer mtime beforehand). The current test predates this parcel and does not exercise a git-reading path; without the stale-index setup it cannot fail for the right reason.
7. `i3_hostile_fsmonitor_config_is_not_executed` — **the important one.** Fixture: a Git repository whose `.git/config` sets `core.fsmonitor` to a script that writes a canary file **outside the scanned root**. Run `repo-radar` with Git enabled. Assert the canary does **not** exist and the report still renders. This test fails against an implementation that omits `-c core.fsmonitor=false`, which is what makes it worth having.
8. `i4_hostile_branch_name_does_not_reach_output_unsanitized` — a branch name containing an ANSI escape; assert no raw escape in text output.
9. `non_git_directory_still_produces_a_report` — AC 4. Exit status 0, and both Git analyses `evaluated: false` with reason `input_unavailable`.

### Integration, in `tests/structured_output.rs`

10. `git_analyses_report_counts_in_json` — against a Git fixture with one modified and one untracked file: `git_status.evaluated == true` and the counts match.
11. `no_git_flag_reports_not_evaluated` — `--no-git` gives `evaluated: false`, `reason == "disabled"`, and zeroed fields, for both analyses.
12. `empty_repository_reports_status_but_not_activity` — status `evaluated: true`, activity `evaluated: false` with reason `input_unavailable`.

### Fixture helpers, in `tests/common/mod.rs`

The existing `git`, `git_fixture`, and `assert_target_unchanged` already do most of this — **extend, do not duplicate**. Add:

- `git_fixture_with_changes()` — a commit, then one modified tracked file, one untracked file, and a `.gitignore` with an ignored path.
- `git_fixture_empty()` — `git init` with no commits.
- `git_fixture_hostile_fsmonitor(canary: &Path)` — for test 7.

Every one returns `Option<Fixture>` and yields `None` when git is unavailable, matching the existing skip-rather-than-fail convention.

## Out of scope

- **The hand-written crate error enum.** The 5b sheet predicted this would land with 4b. **That prediction was wrong, and this sheet supersedes it.** Every Git failure degrades to `NotEvaluated` by design, so nothing in this parcel produces an error that must propagate out of `scan`, and its `io::Result` signature is not forced to change. Adding the enum here would be a refactor with no caller needing it. It lands when something genuinely cannot degrade — most likely 4c, if a malformed manifest turns out to need it, which that sheet must verify rather than assume.
- Branch name, HEAD, remotes, upstream, fork status, license, authorship — all of that is [013 provenance](../specs/013-provenance.md), phase 7. This parcel reports counts and dates, nothing identifying.
- Churn, hotspots, per-author attribution — [021](../specs/021-activity.md), phase 23.
- Submodules, worktrees other than the root, `.git` files pointing elsewhere.
- Cargo manifest parsing — 4c.
- The `scan/`, `analysis/lines.rs`, `analysis/languages.rs`, and `sanitize.rs` extractions.

## Gotchas

1. **`-c core.fsmonitor=false` goes on every invocation, including `log` and `rev-parse`.** Putting it only on `status` is how it gets dropped later when someone adds a fourth command by copying the wrong one.
2. **`-c` arguments must precede the subcommand.** `git -c foo=bar status` works; `git status -c foo=bar` does not.
3. **Bare repositories exit 0.** `rev-parse --is-inside-work-tree` prints `false` and succeeds. Check stdout, not just the status.
4. **`--pretty=format:` omits the trailing newline; use `tformat:`.** Measured.
5. **`git log` exits 128 in a repository with no commits.** Check `rev-parse --verify HEAD` first rather than matching the stderr message.
6. **Never pass git stderr into a `NotEvaluated` detail.** Locale-dependent and repository-influenced. Tool-authored constants only.
7. **`ScanReport` derives `Default`**; both new fields default to `NotEvaluated(Disabled)` via the existing `Analysis` impl. Do not add a `T: Default` bound to `ScanReport`.
8. **`Eq` must stay derivable.** All new fields are integers and `String`. No floats — ENGINEERING.md's C-COMMON-TRAITS note, and a float in a count is a bug anyway.
9. **Sort `by_day` explicitly.** Do not inherit `git log`'s order.
10. **The two `--since-days` boundaries.** `0` is legal and means "today only"; reject a non-numeric value with the existing `parse_arguments` error style. Match how `--top` handles a bad value.
11. **Text output for the existing analyses must stay byte-identical.** The new `Git:` block is additive. `tests/cli.rs` and the existing text assertions must pass unmodified.
12. **Do not weaken `i2_git_state_is_never_mutated`.** Extend it. If it goes red, the implementation is violating I2 — stop and report rather than relaxing the digest.

## Green bar

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Plus, reported in the receipt:

```bash
# Both analyses, ran and disabled.
cargo run -- . --format json | jq -S '.git_status, .git_activity'
cargo run -- . --format json --no-git | jq -S '.git_status, .git_activity'

# A non-Git directory must still exit 0 with a full report.
mkdir -p /tmp/rr-nogit && cargo run -- /tmp/rr-nogit --format json | jq -S '.git_status'; echo "exit=$?"

# Text output, both states.
cargo run -- . --format text | head -20
cargo run -- . --format text --no-git | head -20

# MSRV still holds.
cargo +1.88.0 check --all-targets --all-features
```

## Definition of done

- `ScanReport` carries `git_status` and `git_activity`, both `Analysis<T>`.
- Every reason-mapping row above is reachable and has a test.
- A non-Git directory exits 0 with a complete report (AC 4).
- No user-controlled value ever enters the git argument vector (AC 3); the root is passed via `current_dir`.
- `i3_hostile_fsmonitor_config_is_not_executed` passes, and has been observed to **fail** with the `-c core.fsmonitor=false` removed. A safety test that has never failed proves nothing — ENGINEERING.md's rule — so verify this deliberately and report both results.
- The extended I2 test passes with the Git analyses enabled against a stale-index fixture.
- `tests/cli.rs` passes unmodified; existing text and JSON output unchanged apart from the additive fields.
- `--no-git` and `--since-days` documented in `--help` and the README.
- Spec 003 records 4b as delivered and carries the `core.fsmonitor` finding.
- Green bar clean. No commit, no push.
