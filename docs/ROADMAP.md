# Repo Radar Implementation Roadmap

Status: Active
Updated: 2026-09-05

This roadmap sequences the feature specifications in `docs/specs/`. Each phase must be completed and verified before the next phase begins. The order favors a useful CLI early, then builds toward a responsive interactive code observatory without prematurely committing to a UI framework.

## Product Direction

Repo Radar should answer four questions quickly:

1. What is in this repository?
2. How does it hold together?
3. Where should I look first?
4. What changed or is changing right now?

The CLI remains the stable core. The TUI and web interfaces consume the same library and structured data rather than implementing separate scanners.

## Delivery Order

| Phase | Spec | Expected outcome | Status |
| --- | --- | --- | --- |
| 0 | `SPEC.md` | Human-readable summary works locally | Complete |
| 1 | [001 scan engine](specs/001-scan-engine.md) | Testable library core with deterministic traversal and ignore rules | Complete |
| 2 | [002 structured output](specs/002-structured-output.md) | Stable JSON output for scripts and future UIs | Complete |
| 3 | [007 parallel scanning](specs/007-parallel-scanning.md) | Same results, measurably faster on large trees | Next |
| 4 | [003 repository intelligence](specs/003-repository-intelligence.md) | Language, size, Git, and dependency signals | Planned |
| 5 | [008 code annotations](specs/008-code-annotations.md) | TODO/FIXME harvest and test-surface signals | Planned |
| 6 | [009 symbol index](specs/009-symbol-index.md) | What is defined, and where | Planned |
| 7 | [010 dependency graph](specs/010-dependency-graph.md) | Module and package graphs, cycles, entry points | Planned |
| 8 | [011 incremental cache](specs/011-incremental-cache.md) | Warm scans proportional to what changed | Planned |
| 9 | [004 watch mode](specs/004-watch-mode.md) | Live incremental updates without full rescans | Planned |
| 10 | [005 terminal explorer](specs/005-terminal-explorer.md) | Fast interactive navigation in the terminal | Planned |
| 11 | [012 search index](specs/012-search-index.md) | Interactive-speed content and symbol search | Planned |
| 12 | [006 local web API](specs/006-local-web-api.md) | Optional browser UI on the same local data model | Optional |

## Milestones

### Milestone A: Useful CLI (phases 3-5)

A developer runs one command, exports a report, and identifies the largest, busiest, or most concentrated parts of a repository, along with its unfinished work.

### Milestone B: Structural Map (phases 6-7)

Repo Radar knows what is defined and how modules and packages depend on each other, including cycles and orphaned files.

### Milestone C: Live Observatory (phases 8-9)

Warm scans are near-instant and the report stays open while files change, updating only affected summaries.

### Milestone D: Interactive Tool (phases 10-11)

A terminal user filters, sorts, inspects, searches, and refreshes repository signals without memorizing flags.

### Milestone E: Optional Visual Surface (phase 12)

Built only after the library, TUI, and data model are stable. The web API is local-only and must not become a second product core.

## Rust Learning Focus per Phase

| Phase | Concepts exercised |
| --- | --- |
| 1-2 | Ownership, borrowing, error handling, `serde` derive, module boundaries |
| 3 | `rayon`, data-parallel iterators, avoiding shared mutable state |
| 4-5 | Trait objects, streaming file reads, encoding and binary detection |
| 6-7 | Arena and index-based graphs, lifetimes without reference cycles, topological traversal |
| 8-9 | Serialization formats, atomic file writes, channels, debouncing, `Drop` and cleanup |
| 10-11 | `ratatui` state machines, `mmap` and unsafe boundaries, benchmarking discipline |
| 12 | Async runtimes, graceful shutdown, local-only network surfaces |

## Working Rhythm

Each phase is one or more small parcels:

1. Update the relevant spec with any clarified behavior.
2. Implement one observable slice.
3. Add focused unit and integration tests.
4. Add a benchmark when the phase changes performance-sensitive code.
5. Run `cargo fmt -- --check`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`.
6. Ship the parcel through the `gitify` workflow.

## Phase Exit Checklist

A phase is not complete until all of the following are true:

1. Every acceptance criterion in the phase specification is verified by a test or a documented manual check.
2. The specification `Status` is updated to `Implemented`.
3. The quality gates pass locally and in CI.
4. This roadmap's status column is updated.
5. `README.md` is updated to reflect the shipped state, including its Features, Getting Started, Usage, and Roadmap sections.

## Deliberate Non-Goals

- Cloud synchronization or telemetry
- Editing source files from Repo Radar
- A hosted service
- Language-server-level semantic analysis before the filesystem and Git model are stable
- A large frontend before the local core is reusable
