# Repo Radar

A local code observatory. Repo Radar inspects a repository and explains it: what it is, where it came from, how it is built, how to run it, how it holds together, what it depends on, what shape it is in, and what has been happening in it.

It is built for the moment you open a codebase with no context in your head. That happens three ways, and all three are first-class:

- **Code you cloned** and have never read
- **Code you wrote** and have since forgotten
- **Code you are about to change** and want to understand before touching

Repo Radar is an instrument, not a build tool. It reads, it explains, and it never touches the thing it is measuring.

## The read-only promise

**Repo Radar treats every repository it inspects as immutable, and is always safe to run.**

- It never creates, modifies, deletes, or renames anything inside the scanned repository
- It never mutates Git state — no fetch, no pull, no checkout, no index refresh
- It never executes any code, script, task, or command it finds, including ones it reports back to you
- It performs no network access unless you explicitly ask, and never sends repository content anywhere
- It collects no telemetry, and never will

This is the central product promise, not a limitation of the current release, and **it is enforced rather than asserted**. A test harness digests a fixture tree — contents, sizes, timestamps, permissions, and symlink targets — before and after every command, and fails if anything moved, including on error paths. Thirteen tests hold the ten invariants in [spec 000](docs/specs/000-safety-invariants.md), which also records, honestly, the two criteria not yet fully enforced and why.

The harness is tested against itself: it must detect created, modified, and removed files, and it fails rather than trivially passes when run over an empty tree. A harness that cannot fail would make everything built on it worthless.

You should be able to point Repo Radar at an untrusted clone without reading it first. That is the point.

## Features

Shipped today. Everything else is specified and sequenced in the [roadmap](#roadmap) below.

- **Deterministic recursive scanning** — results do not depend on filesystem directory ordering, so two scans of the same tree are equal.
- **Safe traversal** — symbolic links are never followed, and `.git`, `target`, and `node_modules` are skipped by default.
- **Non-fatal error handling** — an unreadable file or directory becomes a structured warning with its path; the scan continues.
- **Extension and size summary** — file counts grouped by lowercase extension, total byte size, and the largest files in descending order.
- **Human output** — a readable summary with byte sizes formatted as B, KiB, MiB, or GiB.
- **JSON output** — one versioned machine-readable document on stdout, suitable for pipelines and future UIs.
- **Reusable library** — `repo_radar::scan` is callable from Rust without spawning a process, with traversal configurable through `ScanConfig`.
- **Strict argument handling** — an unknown flag, a missing flag value, a bad value, or a second path is a usage error, never a silent fallback.
- **Terminal-safe output** — file names carrying ANSI escape sequences or other control characters are neutralized before display, so a hostile repository cannot recolor, reposition, or hide part of a report.
- **Enforced immutability** — a shared test harness proves every command leaves the scanned repository, and its `.git` directory, byte-identical.

## Getting Started

### Requirements

- Rust stable, edition 2024 (Rust 1.85 or newer). Install with [rustup](https://rustup.rs).

### Install

Build and install the binary onto your `PATH`:

```bash
git clone https://github.com/donco-labs/repo-radar.git
cd repo-radar
cargo install --path .
```

Then run it anywhere:

```bash
repo-radar ~/dev/my-project
```

### Run without installing

```bash
cargo run -- ~/dev/my-project --top 5
```

Everything after `--` is passed to Repo Radar rather than to Cargo.

### First scan

```bash
repo-radar . --top 3
```

```text
Repository: .
Files:      27
Size:       56.2 KiB

Languages / extensions:
  [no extension]   1
  lock             1
  md               18
  rs               5
  toml             1
  yml              1

Largest files:
     7.5 KiB  src/lib.rs
     6.8 KiB  src/main.rs
     4.9 KiB  docs/ROADMAP.md
```

## Usage

```text
repo-radar [PATH] [OPTIONS]

Arguments:
  PATH                  Directory to scan (default: the current directory)

Options:
  --format text|json    Output format (default: text)
  --top N               Number of largest files to list (default: 10)
  -h, --help            Print help and exit
```

### Exit status

| Code | Meaning |
| --- | --- |
| `0` | Success |
| `1` | The path is missing or is not a directory |
| `2` | Invalid usage, such as an unknown flag or a bad flag value |

### JSON output

`--format json` writes exactly one JSON document to stdout. Diagnostics go to stderr, so the document is always safe to pipe.

```bash
repo-radar . --format json --top 2
```

```json
{
  "version": 1,
  "repository": ".",
  "files": 27,
  "bytes": 57567,
  "by_extension": { "md": 18, "rs": 5, "toml": 1 },
  "largest_files": [
    { "path": "src/lib.rs", "bytes": 7700 },
    { "path": "src/main.rs", "bytes": 6998 }
  ],
  "warnings": []
}
```

Pipeline examples:

```bash
# The five largest files, as a table
repo-radar . --format json --top 5 | jq -r '.largest_files[] | "\(.bytes)\t\(.path)"'

# Total Rust files across several checkouts
for repo in ~/dev/*; do
  printf '%s\t%s\n' "$repo" "$(repo-radar "$repo" --format json | jq '.by_extension.rs // 0')"
done

# Fail a check if the scan produced any warnings
repo-radar . --format json | jq -e '.warnings | length == 0' > /dev/null
```

The schema is additive within version `1`. Removing or renaming a field requires a new schema version and a spec update.

### Library use

```rust
use repo_radar::{ScanConfig, scan};
use std::path::Path;

fn main() -> std::io::Result<()> {
    let config = ScanConfig {
        ignored_directories: vec!["target".to_owned(), "dist".to_owned()],
    };
    let report = scan(Path::new("."), &config)?;

    println!("{} files, {} bytes", report.files, report.bytes);
    for file in &report.largest_files {
        println!("{:>10}  {}", file.bytes, file.path.display());
    }
    for warning in &report.warnings {
        eprintln!("{}: {}", warning.path.display(), warning.message);
    }
    Ok(())
}
```

## Roadmap

Repo Radar is built spec-first: every capability below has a written specification with acceptance criteria before any code is written. Phases 0-3 are complete. Full detail, ordering rationale, and per-phase Rust learning goals are in [docs/ROADMAP.md](docs/ROADMAP.md).

### Milestone A — Trustworthy core

| Phase | Feature | Status |
| --- | --- | --- |
| 3 | [Safety invariants](docs/specs/000-safety-invariants.md) — the immutability harness every later phase inherits | **Complete** |
| 4 | [Parallel scanning](docs/specs/007-parallel-scanning.md) — `rayon` fan-out, byte-identical results | Next |
| 5 | [Repository intelligence](docs/specs/003-repository-intelligence.md) — lines, languages, Git basics, Cargo deps | Planned |

### Milestone B — Orientation

The milestone that justifies the tool: `repo-radar brief` tells a stranger what a repository is and how to start.

| Phase | Feature |
| --- | --- |
| 6 | [Provenance](docs/specs/013-provenance.md) — origin, fork status, license, authorship, bus factor |
| 7 | [Project profile](docs/specs/014-project-profile.md) — stated purpose and full tech stack, every finding citing its evidence file |
| 8 | [Runbook](docs/specs/015-runbook.md) — build, run, test, and configure knowledge, extracted and never executed |
| 9 | [Orientation brief](docs/specs/020-brief.md) — **the headline command**, in onboard and resume modes |

### Milestone C — Structure

| Phase | Feature |
| --- | --- |
| 10 | [Code annotations](docs/specs/008-code-annotations.md) — TODO/FIXME harvest and test-surface signals |
| 11 | [Symbol index](docs/specs/009-symbol-index.md) — what is defined, and where |
| 12 | [Incremental cache](docs/specs/011-incremental-cache.md) — warm runs proportional to what changed |
| 13 | [Dependency graph](docs/specs/010-dependency-graph.md) — module and package graphs, cycles, orphans |
| 14 | [Subsystem map](docs/specs/016-subsystem-map.md) — named components you can hold in your head |

### Milestone D — Judgement

| Phase | Feature |
| --- | --- |
| 15 | [Dependency intelligence](docs/specs/017-dependency-intelligence.md) — versions, staleness, SPDX licensing conflicts, alternatives |
| 16 | [Activity and hotspots](docs/specs/021-activity.md) — commit pulse, churn-versus-size risk, change coupling, knowledge gaps |
| 17 | [Health assessment](docs/specs/018-health-assessment.md) — ranked findings, each citing its evidence |

### Milestone E — Visualization

| Phase | Feature |
| --- | --- |
| 18 | [Visual report](docs/specs/019-visual-report.md) — one self-contained HTML file, subsystem diagram, hotspot scatter, treemap, no network |

### Milestone F — Live and interactive

| Phase | Feature |
| --- | --- |
| 19 | [Watch mode](docs/specs/004-watch-mode.md) — live updates as files change |
| 20 | [Search index](docs/specs/012-search-index.md) — fast content and symbol search |
| 21 | [Terminal explorer](docs/specs/005-terminal-explorer.md) — interactive `ratatui` navigation |
| 22 | [Local web API](docs/specs/006-local-web-api.md) — optional loopback-only browser UI |

### The command tree

Where this is heading, as a shape:

```text
repo-radar [PATH]           # scan: what is in here
repo-radar brief [PATH]     # what do I need to know to start
repo-radar run [PATH]       # how do I build, run, and configure it
repo-radar symbols [PATH]   # what is defined, and where
repo-radar graph [PATH]     # how does it hold together
repo-radar map [PATH]       # what are its subsystems
repo-radar deps [PATH]      # what does it depend on, and at what cost
repo-radar activity [PATH]  # what has been happening here
repo-radar health [PATH]    # what is wrong, and what matters most
repo-radar report [PATH]    # show me all of it, visually
repo-radar search QUERY     # where is this thing
repo-radar watch [PATH]     # keep this current as I work
repo-radar tui [PATH]       # let me explore it interactively
repo-radar serve [PATH]     # serve it to a local browser
```

### Deliberate non-goals

- Modifying, formatting, or fixing the repository under inspection
- Executing any code, task, or command found in a repository
- Cloud sync, hosted services, or telemetry of any kind
- Network access as a default behavior
- Presenting a heuristic as a fact, or an absent analysis as a passing one

### Honesty requirements

Repo Radar's value depends on being trusted about what it does not know. Across every feature:

- An analysis that could not run reports `not evaluated`, never a pass
- A heuristic is labelled as one, and states the evidence behind it
- Absent input produces an explicit gap, never a plausible-sounding guess
- Recommendations, such as dependency alternatives or health scores, are labelled as opinion and carry their inputs

## Development

Repo Radar uses spec-driven development. Behavior changes start with a specification update, not with code. See [docs/SDD.md](docs/SDD.md) for the policy and [docs/specs/](docs/specs/) for the 22 feature specifications.

[Spec 000](docs/specs/000-safety-invariants.md) outranks everything else. Any change that could write to a scanned repository, mutate Git state, execute repository content, or reach the network by default must amend it first, in its own reviewed change.

Quality gates, matching CI:

```bash
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Benchmark the scan engine:

```bash
cargo bench --bench scan_engine
```

Run only the safety invariant suite:

```bash
cargo test --test safety_invariants
```

The shared harness is in `tests/common/mod.rs`. Any new command must run its tests inside `assert_target_unchanged`, which fails the test if the command altered the fixture repository in any way.

A phase is not complete until its acceptance criteria are verified, its spec `Status` reads `Implemented`, the roadmap is updated, and this README reflects the shipped state.

## License

MIT
