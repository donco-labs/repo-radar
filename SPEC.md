# Repo Radar Product Specification

Status: Active
Version: 0.1

This document is the authoritative product specification for Repo Radar. Code, tests, documentation, and pull requests must remain consistent with it. When behavior changes, update this specification first or in the same change.

## Purpose

Repo Radar gives developers a fast, local summary of a repository so they can understand its shape before making changes. The first release is a command-line tool built with Rust's standard library.

## User Experience

The command accepts an optional repository path and an optional number of largest files to show:

```text
repo-radar [PATH] [--top N]
```

When no path is supplied, it scans the current directory. The command prints:

- Repository path
- Total regular-file count
- Total file size
- File counts grouped by lowercase extension
- The largest files, descending by byte size

`--help` and `-h` print usage information and exit successfully.

## Safety and Scope

- The scanner must not follow symbolic links.
- The scanner must skip `.git`, `target`, and `node_modules` directories.
- A path that does not exist or is not a directory must produce a concise error and a non-zero exit status.
- The tool must not modify files in the scanned repository.
- The first release has no network access and no telemetry.

## Acceptance Criteria

1. `cargo run -- .` completes successfully in this repository.
2. The reported file count excludes files below skipped directories.
3. Extension counts are case-insensitive and files without an extension are grouped as `[no extension]`.
4. `--top N` limits the largest-file list to at most `N` entries.
5. `cargo test` passes.
6. `cargo clippy --all-targets --all-features -- -D warnings` passes.

## Planned Evolution

Future capabilities require a new or revised spec before implementation. The sequenced feature specifications and delivery timeline live in [docs/ROADMAP.md](docs/ROADMAP.md):

- [Scan engine](docs/specs/001-scan-engine.md)
- [Structured output](docs/specs/002-structured-output.md)
- [Repository intelligence](docs/specs/003-repository-intelligence.md)
- [Watch mode](docs/specs/004-watch-mode.md)
- [Terminal explorer](docs/specs/005-terminal-explorer.md)
- [Optional local web API](docs/specs/006-local-web-api.md)