# Repo Radar Product Specification

Status: Active
Version: 0.2

This document is the authoritative product specification for Repo Radar. Code, tests, documentation, and pull requests must remain consistent with it. When behavior changes, update this specification first or in the same change.

The safety invariants in [000 safety invariants](docs/specs/000-safety-invariants.md) outrank this document. Where this specification appears to permit a violation of them, they win.

## Purpose

Repo Radar is a local code observatory. It inspects a repository and explains it: what it is, where it came from, how it is built, how to run it, how it holds together, what it depends on, what shape it is in, and what has been happening in it.

It exists for the moment a developer opens a codebase without context in their head. That happens in three ways, and all three are first-class:

- Code cloned from elsewhere that the developer has never read
- Code the developer wrote and has since forgotten
- Code the developer is about to change and wants to understand before touching

Repo Radar is an instrument, not a build tool. It reads, it explains, and it never touches the thing it is measuring.

## The Read-Only Promise

**Repo Radar treats every repository it inspects as immutable, and is always safe to run.**

It never creates, modifies, deletes, or renames anything inside the scanned repository. It never mutates Git state. It never executes any code, script, task, or command that it finds. It performs no network access unless explicitly asked, and it collects no telemetry under any circumstances.

This is the product's central promise, not a limitation of the current release. It is enforced by the invariants and the mandatory test harness in [000 safety invariants](docs/specs/000-safety-invariants.md), which asserts a fixture tree is byte-identical before and after every command, including on failure paths.

A user must be able to run Repo Radar against an untrusted clone without reading it first. Any change that weakens that requires amending spec 000 in its own reviewed change.

## User Experience

Repo Radar is a command tree. The bare form scans, so the simplest use needs no subcommand:

```text
repo-radar [PATH] [--format text|json] [--top N]
```

Named commands expose the deeper analyses. Each is specified by the feature specification named beside it:

| Command | Question it answers | Spec |
| --- | --- | --- |
| `scan` (default) | What is in this repository? | 001, 002 |
| `brief` | What do I need to know to start? | 020 |
| `run` | How do I build, run, and configure it? | 015 |
| `symbols` | What is defined, and where? | 009 |
| `graph` | How does it hold together? | 010 |
| `map` | What are its subsystems? | 016 |
| `deps` | What does it depend on, and at what cost? | 017 |
| `activity` | What has been happening here? | 021 |
| `health` | What is wrong, and what matters most? | 018 |
| `report` | Show me all of it, visually. | 019 |
| `search` | Where is this thing? | 012 |
| `watch` | Keep this current as I work. | 004 |
| `tui` | Let me explore it interactively. | 005 |
| `serve` | Serve it to a local browser. | 006 |

When no path is supplied, the current directory is scanned. The default output format is `text`; `json` is the machine contract.

`--help` and `-h` print usage for the tool or for a named command and exit successfully. Help output is authoritative: every accepted flag appears in it.

As the command tree grows beyond two commands, argument parsing may adopt an established parser crate. The observable interface defined here does not change when it does.

## Output Contract

Text output is for humans and its formatting is not a stability guarantee.

JSON output is the stable contract that scripts, the terminal explorer, and the web interface all consume. It carries a schema version starting at `1`, and is additive within a version: removing or renaming a field requires a new version and a spec update. There is exactly one JSON document on stdout; diagnostics go to stderr.

Every reported finding must name the evidence that produced it. A claim Repo Radar cannot trace to a file, a line, or a commit does not ship.

## Safety and Scope

- The scanner must not follow symbolic links.
- The scanner must skip `.git`, `target`, and `node_modules` by default.
- A path that does not exist or is not a directory must produce a concise error and exit status `1`.
- An invalid flag value, an unknown flag, a flag missing its value, or more than one path argument must produce a concise error and exit status `2`, never a silent fallback to a default.
- Repository content is untrusted input and is never interpolated into a shell command.
- The tool must not modify files in the scanned repository, under any command or failure path.
- Network access is opt-in per invocation, never implied, and never transmits repository content.
- There is no telemetry.

## Honesty Requirements

Repo Radar's value depends on being trusted about what it does not know.

- An analysis that could not run reports `not evaluated`, never a pass.
- A heuristic is labelled as one, and states the evidence behind it.
- Absent input produces an explicit gap, never a plausible-sounding guess.
- Recommendations, such as dependency alternatives or health scores, are labelled as opinion and carry their inputs.

## Acceptance Criteria

1. `cargo run -- .` completes successfully in this repository.
2. The reported file count excludes files below skipped directories.
3. Extension counts are case-insensitive and files without an extension are grouped as `[no extension]`.
4. `--top N` limits the largest-file list to at most `N` entries, and a non-numeric `N` exits with status `2`.
5. `--format json` emits one valid JSON document on stdout and nothing on stderr.
6. `--help` output names every supported flag.
7. Every command upholds the invariants of spec 000 under its mandatory test harness.
8. `cargo test` passes.
9. `cargo clippy --all-targets --all-features -- -D warnings` passes.

## Planned Evolution

Future capabilities require a new or revised spec before implementation. The sequenced specifications and delivery order live in [docs/ROADMAP.md](docs/ROADMAP.md).

**Foundation**

- [000 Safety invariants](docs/specs/000-safety-invariants.md) — active
- [001 Scan engine](docs/specs/001-scan-engine.md) — implemented
- [002 Structured output](docs/specs/002-structured-output.md) — implemented
- [007 Parallel scanning](docs/specs/007-parallel-scanning.md)
- [003 Repository intelligence](docs/specs/003-repository-intelligence.md)

**Orientation**

- [013 Provenance and identity](docs/specs/013-provenance.md)
- [014 Project profile](docs/specs/014-project-profile.md)
- [015 Runbook extraction](docs/specs/015-runbook.md)
- [020 Orientation brief](docs/specs/020-brief.md)

**Structure**

- [008 Code annotations and test signals](docs/specs/008-code-annotations.md)
- [009 Symbol index](docs/specs/009-symbol-index.md)
- [011 Incremental cache](docs/specs/011-incremental-cache.md)
- [010 Dependency graph](docs/specs/010-dependency-graph.md)
- [016 Subsystem map](docs/specs/016-subsystem-map.md)

**Judgement**

- [017 Dependency intelligence](docs/specs/017-dependency-intelligence.md)
- [021 Activity and hotspots](docs/specs/021-activity.md)
- [018 Health assessment](docs/specs/018-health-assessment.md)

**Surfaces**

- [019 Visual report](docs/specs/019-visual-report.md)
- [004 Watch mode](docs/specs/004-watch-mode.md)
- [012 Search index](docs/specs/012-search-index.md)
- [005 Terminal explorer](docs/specs/005-terminal-explorer.md)
- [006 Optional local web API](docs/specs/006-local-web-api.md)
