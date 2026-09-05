# Repo Radar

A local code observatory. Repo Radar inspects a repository and explains it: what it is, where it came from, how it is built, how to run it, how it holds together, what it depends on, what shape it is in, what has been happening in it, and who has been changing it.

It is built for the moment you open a codebase with no context in your head. That happens four ways, and all four are first-class:

- **Code you cloned** and have never read
- **Code you wrote** and have since forgotten
- **Code you are about to change** and want to understand before touching
- **Code being written right now**, by an agent, while you watch

Repo Radar is an instrument, not a build tool. It reads, it explains, and it never touches the thing it is measuring.

### What makes it different

Counting files by extension is solved, and `cloc`, `tokei`, and `scc` solve it well. Repo Radar aims at two things those tools and a forge's insights tab do not reach:

- **It is live.** A repository under active development is a moving target, and a report written once is stale on arrival. The primary rich surface is `repo-radar serve` — one command that scans, binds a loopback port, opens a browser, and streams the model as the repository changes. See [spec 006](docs/specs/006-local-web-api.md).
- **It reads authorship process, not just state.** Most non-trivial code is now written with agentic assistance, and that process leaves structured local traces. With `--agents`, Repo Radar reads them and reports what an agent touched, in what order, how fast, what it rewrote, and what it changed without ever reading — live, while it works. Structural events only: no prompt text, no model output, no network. See [spec 022](docs/specs/022-agent-activity.md).

Both are specified and sequenced, not yet built. What is shipped today is listed under Features below, and nothing else is claimed.

### The UI is Rust

The browser surface is a [Dioxus](https://dioxuslabs.com) component tree compiled to `wasm32-unknown-unknown` and embedded in the binary — no separate frontend project, no npm, no asset directory, and no hand-written TypeScript interface that can drift from the Rust model. The same component tree renders to a native WebView, so a desktop shell is a renderer swap rather than a second application. Architecture and the pinned version are in [spec 024](docs/specs/024-view-layer.md).

The server underneath it is deliberately *not* a framework: `std::net::TcpListener`, a bounded thread pool, and server-sent events written by hand. The scan engine is synchronous, and adding an async runtime to serve a local dashboard would colour the whole crate async to solve a problem it does not have.

The terminal explorer keeps [`ratatui`](https://ratatui.rs). Unifying it under the same component tree was the strongest argument for Dioxus, but its terminal renderer sits at `0.5.0-alpha.0` against a `0.7.10` core and its own docs link still points at 0.4 — so Repo Radar accepts two view layers over one model rather than one view layer over an abandoned dependency. [Spec 005](docs/specs/005-terminal-explorer.md) records that decision and the evidence for it.

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
- **HTML dashboard** — a self-contained visual snapshot with composition bars, largest files, and scan notes.
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
  --format text|json|html
                        Output format (default: text; html is a standalone dashboard)
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

### HTML dashboard

Generate a visual snapshot that can be opened directly in a browser. It has no external assets or network requests:

```bash
cargo run -- . --format html --top 8 > /tmp/repo-radar.html
open /tmp/repo-radar.html
```

The dashboard is an early visual surface over the same scan model as text and JSON. As repository intelligence lands, it will grow with those fields rather than maintaining a separate UI model.

Note what this is and is not. A file written to `/tmp` is a *snapshot*: stale the moment it is written, and unable to show anything that moves. It stays, because attaching a frozen view to a review is a real job. But it is not the intended way to look at a repository — that is `repo-radar serve`, which needs no redirect, no temporary path, and no manual reload, and which streams updates as the repository changes. It is specified in [spec 006](docs/specs/006-local-web-api.md) and scheduled as phase 10.

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

Repo Radar is built spec-first: every capability below has a written specification with acceptance criteria before any code is written, and [docs/ENGINEERING.md](docs/ENGINEERING.md) states how the code implementing them is built. Phases 0-3 — the scan engine, the JSON contract, and the enforced immutability harness — are complete. Full detail, ordering rationale, and per-phase Rust learning goals are in [docs/ROADMAP.md](docs/ROADMAP.md).

### Milestone A — Orientation

Composition stops being a list of file extensions: languages ranked by source bytes, directory weight, a stated purpose with the file it came from, and where the code came from.

| Phase | Feature | Status |
| --- | --- | --- |
| 4 | [Repository intelligence](docs/specs/003-repository-intelligence.md) — lines, languages, largest directories, Git basics, Cargo deps | In progress |
| 5 | [Engineering guidelines](docs/ENGINEERING.md) — module tree, the `Analysis` seam, crate lints, declared MSRV | Planned |
| 6 | [Project profile](docs/specs/014-project-profile.md) — stated purpose from the manifest or README, and full tech stack, every finding citing its evidence file | Planned |
| 7 | [Provenance](docs/specs/013-provenance.md) — origin, fork status, license, authorship, bus factor | Planned |
| 8 | [Runbook](docs/specs/015-runbook.md) — build, run, test, and configure knowledge, extracted and never executed | Planned |
| 9 | [Orientation brief](docs/specs/020-brief.md) — **the headline command**, in onboard and resume modes | Planned |

### Milestone B — Live

One command, a real surface, current while you work.

| Phase | Feature |
| --- | --- |
| 10 | [Watch mode](docs/specs/004-watch-mode.md) — debounced refresh as files change |
| 11 | [Local live surface](docs/specs/006-local-web-api.md) — the `serve` transport: loopback-only, server-sent events, hand-rolled on `std::net::TcpListener` with no async runtime and no framework |
| 12 | [View layer](docs/specs/024-view-layer.md) — **the UI, written in Rust**: a Dioxus component tree compiled to wasm, embedded in the binary, driven by signals off the SSE stream |

### Milestone C — The differentiator

| Phase | Feature |
| --- | --- |
| 13 | [Agent activity](docs/specs/022-agent-activity.md) — **watch an agent build, in real time**: files touched, edit velocity, rework rate, writes to files never read, changes with no test touched. Vendor-neutral event model with per-agent adapters |
| 14 | [Agentic readiness](docs/specs/026-agentic-readiness.md) — **how the repo is set up for agents**: subagents, skills, hooks, MCP servers, permission breadth, committed-credential detection, and whether the work is governed by specs, plan artifacts, or conversation alone. Plus the comparison only we can make — configured versus actually used |
| 15 | [Forge metadata](docs/specs/023-forge-metadata.md) — description, topics, stars, forks, open issues, releases, archived state, and how far this clone has drifted. Opt-in `--network`, sends the repository name and nothing else |

### Milestone D — Structure

| Phase | Feature |
| --- | --- |
| 16 | [Code annotations](docs/specs/008-code-annotations.md) — TODO/FIXME harvest and test-surface signals |
| 17 | [Symbol index](docs/specs/009-symbol-index.md) — what is defined, and where |
| 18 | [Parallel scanning](docs/specs/007-parallel-scanning.md) — `rayon` fan-out, byte-identical results |
| 19 | [Incremental cache](docs/specs/011-incremental-cache.md) — warm runs proportional to what changed |
| 20 | [Dependency graph](docs/specs/010-dependency-graph.md) — module and package graphs, cycles, orphans |
| 21 | [Subsystem map](docs/specs/016-subsystem-map.md) — named components you can hold in your head |

### Milestone E — Judgement

| Phase | Feature |
| --- | --- |
| 22 | [Dependency intelligence](docs/specs/017-dependency-intelligence.md) — versions, staleness, SPDX licensing conflicts, alternatives |
| 23 | [Activity and hotspots](docs/specs/021-activity.md) — commit pulse, churn-versus-size risk, change coupling, knowledge gaps |
| 24 | [Practice assessment](docs/specs/025-practice-assessment.md) — **how well is it built**: structure, coupling, test posture, toolchain gates, error handling. Every finding cites its evidence and the threshold that produced it. No score, no grade, no ranking — and this repository is its first fixture |
| 25 | [Health assessment](docs/specs/018-health-assessment.md) — ranked findings, each citing its evidence |

### Milestone F — More surfaces

| Phase | Feature |
| --- | --- |
| 26 | [Visual report](docs/specs/019-visual-report.md) — one self-contained HTML file to attach to a review: subsystem diagram, hotspot scatter, treemap, no network |
| 27 | [Search index](docs/specs/012-search-index.md) — fast content and symbol search |
| 28 | [Terminal explorer](docs/specs/005-terminal-explorer.md) — interactive `ratatui` navigation |
| 29 | [Desktop shell](docs/specs/024-view-layer.md) — the phase 12 component tree in a native WebView, for the cost of a renderer swap |

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
repo-radar activity [PATH]  # what has been happening here, and who did it
repo-radar agentic [PATH]   # how is it set up for agents
repo-radar practices [PATH] # how well is it built
repo-radar health [PATH]    # what is wrong, and what matters most
repo-radar report [PATH]    # give me a snapshot I can share
repo-radar search QUERY     # where is this thing
repo-radar watch [PATH]     # keep this current as I work
repo-radar tui [PATH]       # let me explore it interactively
repo-radar serve [PATH]     # show me all of it, live, in a browser
```

Two flags widen what the tool touches, so both are opt-in per invocation and neither is ever implied:

```text
--agents     # read local agent session logs, to report authorship process
--network    # ask the origin forge for this repository's public metadata
```

### Deliberate non-goals

- Modifying, formatting, or fixing the repository under inspection
- Executing any code, task, or command found in a repository, or recorded in an agent log
- Ingesting prompt text or model output from agent sessions
- Cloud sync, hosted services, authentication, or telemetry of any kind
- Network access as a default behavior, or transmitting repository content anywhere
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
