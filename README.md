# Repo Radar

A fast, local code observatory. Repo Radar scans a repository and reports its shape — file counts, sizes, language mix, and the largest files — so you can understand a codebase before you change it.

Everything runs on your machine. There is no network access, no telemetry, and Repo Radar never writes to the repository it scans.

This is also a Rust learning project, built spec-first. [SPEC.md](SPEC.md) is the authoritative product behavior, [docs/SDD.md](docs/SDD.md) defines the development process and quality gates, and [docs/ROADMAP.md](docs/ROADMAP.md) sequences the work.

## Features

Shipped today:

- **Deterministic recursive scanning** — results do not depend on filesystem directory ordering, so two scans of the same tree are equal.
- **Safe traversal** — symbolic links are never followed, and `.git`, `target`, and `node_modules` are skipped by default.
- **Non-fatal error handling** — an unreadable file or directory becomes a structured warning with its path; the scan continues.
- **Extension and size summary** — file counts grouped by lowercase extension, total byte size, and the largest files in descending order.
- **Human output** — a readable summary with byte sizes formatted as B, KiB, MiB, or GiB.
- **JSON output** — one versioned machine-readable document on stdout, suitable for pipelines and future UIs.
- **Reusable library** — `repo_radar::scan` is callable from Rust without spawning a process, with traversal configurable through `ScanConfig`.
- **Strict argument handling** — an unknown flag, a missing flag value, a bad value, or a second path is a usage error, never a silent fallback.

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

Repo Radar is being built toward an interactive local code observatory. Phases 0-2 are complete. Full detail, including per-phase Rust learning goals, is in [docs/ROADMAP.md](docs/ROADMAP.md).

| Phase | Feature | Status |
| --- | --- | --- |
| 1 | [Scan engine](docs/specs/001-scan-engine.md) | Complete |
| 2 | [Structured output](docs/specs/002-structured-output.md) | Complete |
| 3 | [Parallel scanning](docs/specs/007-parallel-scanning.md) | Next |
| 4 | [Repository intelligence](docs/specs/003-repository-intelligence.md) — lines, languages, Git activity, Cargo dependencies | Planned |
| 5 | [Code annotations](docs/specs/008-code-annotations.md) — TODO/FIXME harvest and test signals | Planned |
| 6 | [Symbol index](docs/specs/009-symbol-index.md) — what is defined, and where | Planned |
| 7 | [Dependency graph](docs/specs/010-dependency-graph.md) — module and package graphs, cycles, entry points | Planned |
| 8 | [Incremental cache](docs/specs/011-incremental-cache.md) — warm scans proportional to what changed | Planned |
| 9 | [Watch mode](docs/specs/004-watch-mode.md) — live updates as files change | Planned |
| 10 | [Terminal explorer](docs/specs/005-terminal-explorer.md) — interactive `ratatui` navigation | Planned |
| 11 | [Search index](docs/specs/012-search-index.md) — fast content and symbol search | Planned |
| 12 | [Local web API](docs/specs/006-local-web-api.md) — optional loopback-only browser UI | Optional |

Deliberate non-goals: cloud sync, telemetry, editing your source, a hosted service, and a large frontend before the local core is reusable.

## Development

Repo Radar uses spec-driven development. Behavior changes start with a specification update, not with code. See [docs/SDD.md](docs/SDD.md).

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

A phase is not complete until its acceptance criteria are verified, its spec `Status` reads `Implemented`, the roadmap is updated, and this README reflects the shipped state.

## License

MIT
