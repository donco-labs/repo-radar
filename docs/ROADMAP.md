# Repo Radar Implementation Roadmap

Status: Proposed
Updated: 2026-09-05

This roadmap sequences the feature specifications in `docs/specs/`. Each phase should be completed and verified before the next phase begins. The order favors a useful CLI early, then builds toward a responsive interactive code observatory without prematurely committing to a UI framework.

## Product Direction

Repo Radar should answer three questions quickly:

1. What is in this repository?
2. Where should I look first?
3. What changed or is changing right now?

The CLI remains the stable core. TUI and web interfaces consume the same library and structured data rather than implementing separate scanners.

## Delivery Order

| Phase | Target | Spec | Expected outcome |
| --- | --- | --- | --- |
| 0 | Current baseline | `SPEC.md` | Human-readable summary works locally |
| 1 | Weeks 1-2 | [001 scan engine](specs/001-scan-engine.md) | Testable library core with deterministic traversal and ignore rules |
| 2 | Week 3 | [002 structured output](specs/002-structured-output.md) | Stable JSON output for scripts and future UIs |
| 3 | Weeks 4-5 | [003 repository intelligence](specs/003-repository-intelligence.md) | Useful language, size, Git, and dependency signals |
| 4 | Week 6 | [004 watch mode](specs/004-watch-mode.md) | Live incremental updates without rescanning everything |
| 5 | Weeks 7-8 | [005 terminal explorer](specs/005-terminal-explorer.md) | Fast interactive navigation in the terminal |
| 6 | Optional weeks 9-10 | [006 local web API](specs/006-local-web-api.md) | Browser UI built on the same local data model |

## Milestones

### Milestone A: Useful CLI

Complete phases 1-3. A developer can run one command, export a report, and identify the largest, busiest, or most concentrated parts of a repository.

### Milestone B: Live Observatory

Complete phase 4. The report stays open while files change and updates only affected summaries.

### Milestone C: Interactive Tool

Complete phase 5. A terminal user can filter, sort, inspect, and refresh repository signals without memorizing flags.

### Milestone D: Optional Visual Surface

Complete phase 6 only after the library and TUI prove the data model. The web API is local-only and should not become a second product core.

## Working Rhythm

Each phase is one or more small parcels:

1. Update the relevant spec with any clarified behavior.
2. Implement one observable slice.
3. Add focused unit and integration tests.
4. Add a benchmark when the phase changes performance-sensitive code.
5. Run `cargo fmt -- --check`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`.
6. Ship the parcel through the `gitify` workflow.

## Deliberate Non-Goals

- Cloud synchronization or telemetry
- Editing source files from Repo Radar
- A hosted service
- Language-server-level semantic analysis before the filesystem and Git model are stable
- A large frontend before the local core is reusable