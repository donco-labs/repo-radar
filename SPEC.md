# Repo Radar Product Specification

Status: Active
Version: 0.1

This document is the authoritative product specification for Repo Radar. Code, tests, documentation, and pull requests must remain consistent with it. When behavior changes, update this specification first or in the same change.

## Purpose

Repo Radar gives developers a fast, local summary of a repository so they can understand its shape before making changes. The first release is a command-line tool built with Rust's standard library.

## User Experience

The command accepts an optional repository path, an output format, and a number of largest files to show:

```text
repo-radar [PATH] [--format text|json] [--top N]
```

When no path is supplied, it scans the current directory. The default format is `text`, which prints:

- Repository path
- Total regular-file count
- Total file size
- File counts grouped by lowercase extension
- The largest files, descending by byte size

The `json` format emits a single machine-readable document on stdout under the contract in [002 structured output](docs/specs/002-structured-output.md).

`--help` and `-h` print usage information for every supported flag and exit successfully.

## Safety and Scope

- The scanner must not follow symbolic links.
- The scanner must skip `.git`, `target`, and `node_modules` directories.
- A path that does not exist or is not a directory must produce a concise error and exit status `1`.
- An invalid flag value, an unknown flag, a flag missing its value, or more than one path argument must produce a concise error and exit status `2`, never a silent fallback to a default.
- The tool must not modify files in the scanned repository.
- The first release has no network access and no telemetry.

## Acceptance Criteria

1. `cargo run -- .` completes successfully in this repository.
2. The reported file count excludes files below skipped directories.
3. Extension counts are case-insensitive and files without an extension are grouped as `[no extension]`.
4. `--top N` limits the largest-file list to at most `N` entries, and a non-numeric `N` exits with status `2`.
5. `--format json` emits one valid JSON document on stdout and nothing on stderr.
6. `--help` output names every supported flag.
7. `cargo test` passes.
8. `cargo clippy --all-targets --all-features -- -D warnings` passes.

## Planned Evolution

Future capabilities require a new or revised spec before implementation. The sequenced feature specifications and delivery timeline live in [docs/ROADMAP.md](docs/ROADMAP.md):

- [001 Scan engine](docs/specs/001-scan-engine.md) — implemented
- [002 Structured output](docs/specs/002-structured-output.md) — implemented
- [007 Parallel scanning](docs/specs/007-parallel-scanning.md)
- [003 Repository intelligence](docs/specs/003-repository-intelligence.md)
- [008 Code annotations and test signals](docs/specs/008-code-annotations.md)
- [009 Symbol index](docs/specs/009-symbol-index.md)
- [010 Dependency graph](docs/specs/010-dependency-graph.md)
- [011 Incremental cache](docs/specs/011-incremental-cache.md)
- [004 Watch mode](docs/specs/004-watch-mode.md)
- [005 Terminal explorer](docs/specs/005-terminal-explorer.md)
- [012 Search index](docs/specs/012-search-index.md)
- [006 Optional local web API](docs/specs/006-local-web-api.md)