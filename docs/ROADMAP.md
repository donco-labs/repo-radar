# Repo Radar Implementation Roadmap

Status: Active
Updated: 2026-09-05

This roadmap sequences the feature specifications in `docs/specs/`. Each phase must be completed and verified before the next begins. The order front-loads the orientation value — telling a developer what a repository is and how to run it — then deepens into structure, judgement, and interactive surfaces.

## Product Direction

Repo Radar answers eight questions about a repository the user has no context on, whether they cloned it or wrote it and forgot it:

1. What is this, and where did it come from?
2. What is it built with?
3. How do I run it?
4. What is in it?
5. How does it hold together?
6. What does it depend on, and at what cost?
7. What shape is it in?
8. What has been happening, and what changed since I last looked?

The CLI and its library are the stable core. The TUI, the visual report, and the web interface are consumers of the same model. No surface may fork the data model or add analysis logic of its own.

Every phase inherits the read-only invariants of [000 safety invariants](specs/000-safety-invariants.md), including its mandatory before-and-after test harness.

## Delivery Order

| Phase | Spec | Outcome | Status |
| --- | --- | --- | --- |
| 0 | `SPEC.md` | Human-readable summary works locally | Complete |
| 1 | [001 scan engine](specs/001-scan-engine.md) | Deterministic, testable library core | Complete |
| 2 | [002 structured output](specs/002-structured-output.md) | Stable JSON contract, strict CLI | Complete |
| 3 | [000 safety invariants](specs/000-safety-invariants.md) | Immutability harness every later phase reuses | Complete |
| 4 | [007 parallel scanning](specs/007-parallel-scanning.md) | Same results, measurably faster | Next |
| 5 | [003 repository intelligence](specs/003-repository-intelligence.md) | Lines, languages, Git basics, Cargo deps | Planned |
| 6 | [013 provenance](specs/013-provenance.md) | Origin, fork status, license, authorship | Planned |
| 7 | [014 project profile](specs/014-project-profile.md) | Purpose and full tech stack, with evidence | Planned |
| 8 | [015 runbook](specs/015-runbook.md) | Build, run, test, and configure knowledge | Planned |
| 9 | [020 orientation brief](specs/020-brief.md) | **The headline command**, onboard and resume modes | Planned |
| 10 | [008 code annotations](specs/008-code-annotations.md) | TODO harvest and test-surface signals | Planned |
| 11 | [009 symbol index](specs/009-symbol-index.md) | What is defined, and where | Planned |
| 12 | [011 incremental cache](specs/011-incremental-cache.md) | Warm runs proportional to what changed | Planned |
| 13 | [010 dependency graph](specs/010-dependency-graph.md) | Module and package graphs, cycles, orphans | Planned |
| 14 | [016 subsystem map](specs/016-subsystem-map.md) | Named components a person can hold in mind | Planned |
| 15 | [017 dependency intelligence](specs/017-dependency-intelligence.md) | Versions, staleness, licenses, alternatives | Planned |
| 16 | [021 activity and hotspots](specs/021-activity.md) | Pulse, churn-versus-size risk, knowledge gaps | Planned |
| 17 | [018 health assessment](specs/018-health-assessment.md) | Ranked findings with evidence | Planned |
| 18 | [019 visual report](specs/019-visual-report.md) | Self-contained interactive HTML | Planned |
| 19 | [004 watch mode](specs/004-watch-mode.md) | Live updates as files change | Planned |
| 20 | [012 search index](specs/012-search-index.md) | Interactive-speed content and symbol search | Planned |
| 21 | [005 terminal explorer](specs/005-terminal-explorer.md) | Interactive `ratatui` navigation | Planned |
| 22 | [006 local web API](specs/006-local-web-api.md) | Optional loopback-only browser UI | Optional |

## Milestones

### Milestone A: Trustworthy Core (phases 3-5)

The read-only promise is mechanically enforced, scanning is parallel, and the base signals are in place. Nothing after this point may weaken the invariants.

### Milestone B: Orientation (phases 6-9)

**The product becomes useful to a stranger.** `repo-radar brief` answers what a repository is, what it is built with, how to run it, and where to start reading. This is the milestone that justifies the tool.

### Milestone C: Structure (phases 10-14)

Repo Radar understands what is defined, how modules depend on each other, and how the repository decomposes into named subsystems.

### Milestone D: Judgement (phases 15-17)

Inventory becomes assessment: dependency staleness and licensing, churn-versus-size hotspots, and ranked health findings that each cite their evidence.

### Milestone E: Visualization (phase 18)

One self-contained HTML file that renders the whole model, openable from `file://` with no network.

### Milestone F: Live and Interactive (phases 19-22)

The report stays current while work happens, search is instant, and the terminal and browser surfaces make it explorable.

## Ordering Rationale

- **Safety first.** Phase 3 exists before any feature that reads Git or parses manifests, so every later phase inherits a harness that proves it did not mutate the target. Retrofitting that guarantee is far harder than starting with it.
- **Parallelism before analysis.** Phase 4 lands while the scan is simple, so the concurrency model is established before per-file analyses multiply the work.
- **The brief ships early and grows.** Phase 9 arrives before symbols, graphs, and health exist, because spec 020 requires graceful composition. Shipping it at its thinnest proves the composition rule, and every later phase enriches it automatically.
- **Cache before graph.** Phase 12 precedes the expensive graph and subsystem work so those phases are designed against a warm-run model rather than retrofitted into one.
- **Judgement after structure.** Dependency intelligence and health both need the subsystem map to attribute findings to a component rather than to a path.
- **Surfaces last.** The TUI and web UI are consumers. Building them before the model is stable would fork it.

## Rust Learning Focus per Phase

| Phase | Concepts exercised |
| --- | --- |
| 1-2 | Ownership, borrowing, error handling, `serde` derive, module boundaries |
| 3 | Test harness design, `Drop` guards, property-style invariant assertions |
| 4 | `rayon`, data-parallel iterators, avoiding shared mutable state |
| 5-8 | Trait objects, streaming reads, encoding detection, table-driven parsing |
| 9 | Composition over optional data, builder patterns, graceful degradation |
| 10-11 | Line-oriented extraction, versioned static data tables |
| 12 | Serialization formats, atomic writes, cache invalidation, file locking |
| 13-14 | Arena and index-based graphs, lifetimes without reference cycles, clustering |
| 15-17 | Semantic version comparison, optional network with timeouts, scoring models |
| 18 | Deterministic layout, escaping untrusted content, SVG generation |
| 19-20 | Channels, debouncing, `mmap` and unsafe boundaries, benchmarking discipline |
| 21-22 | `ratatui` state machines, async runtimes, graceful shutdown, loopback-only surfaces |

## Working Rhythm

Each phase is one or more small parcels:

1. Update the relevant spec with any clarified behavior.
2. Implement one observable slice.
3. Add focused unit and integration tests, including the spec 000 immutability harness.
4. Add a benchmark when the phase changes performance-sensitive code.
5. Run `cargo fmt -- --check`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`.
6. Ship the parcel through the `gitify` workflow.

## Phase Exit Checklist

A phase is not complete until all of the following are true:

1. Every acceptance criterion in the phase specification is verified by a test or a documented manual check.
2. Any command added or changed upholds the spec 000 invariants under the immutability harness.
3. The specification `Status` is updated to `Implemented`.
4. The quality gates pass locally and in CI.
5. This roadmap's status column is updated.
6. `README.md` is updated to reflect the shipped state, including its Features, Getting Started, Usage, and Roadmap sections.

## Deliberate Non-Goals

- Modifying, formatting, or fixing the repository under inspection
- Executing any code, task, or command found in a repository
- Cloud synchronization, hosted services, or telemetry of any kind
- Network access as a default behavior
- Language-server-level semantic analysis before the filesystem and Git model are stable
- A large frontend before the local core is reusable
- Presenting a heuristic as a fact, or an absent analysis as a passing one
