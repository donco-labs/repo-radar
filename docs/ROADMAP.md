# Repo Radar Implementation Roadmap

Status: Active
Updated: 2026-09-05

This roadmap sequences the feature specifications in `docs/specs/`. Each phase must be completed and verified before the next begins. The order front-loads content — telling a developer what a repository actually is — then makes that content live, then explains who has been changing it.

## Product Direction

Repo Radar answers ten questions about a repository the user has no context on, whether they cloned it, wrote it and forgot it, or watched an agent write it:

1. What is this, and where did it come from?
2. What is it built with?
3. How do I run it?
4. What is in it?
5. How does it hold together?
6. What does it depend on, and at what cost?
7. What shape is it in?
8. What has been happening, and what changed since I last looked?
9. **Who has been changing it, how, and how fast — including when the author is an agent?**
10. **Is it set up to be worked on by agents, and how is that work governed?**

The CLI and its library are the stable core. The live surface, the TUI, and the static report are consumers of the same model. No surface may fork the data model or add analysis logic of its own.

How the code itself is built — module structure, coupling rules, Rust idiom, and the lint posture every parcel is held to — is in [ENGINEERING.md](ENGINEERING.md).

Every phase inherits the read-only invariants of [000 safety invariants](specs/000-safety-invariants.md), including its mandatory before-and-after test harness.

## What Differentiates This

Counting files by extension is a solved problem, and `cloc`, `tokei`, and `scc` solve it well. Repo Radar earns its existence in two places neither those tools nor a forge's insights tab reach:

- **Live.** A repository under active development is a moving target. The default rich surface is a served, streaming view ([006](specs/006-local-web-api.md)) rather than a file written once and stale on arrival.
- **Authorship process.** Most non-trivial code is now written with agentic assistance, and that process leaves structured local traces. [022 agent activity](specs/022-agent-activity.md) reads them: what an agent touched, in what order, how fast, what it rewrote, and what it changed without ever reading. That is the analysis no state-describing tool can produce.

Everything before those phases exists to give them something worth showing.

## Delivery Order

| Phase | Spec | Outcome | Status |
| --- | --- | --- | --- |
| 0 | `SPEC.md` | Human-readable summary works locally | Complete |
| 1 | [001 scan engine](specs/001-scan-engine.md) | Deterministic, testable library core | Complete |
| 2 | [002 structured output](specs/002-structured-output.md) | Stable JSON contract, strict CLI | Complete |
| 3 | [000 safety invariants](specs/000-safety-invariants.md) | Immutability harness every later phase reuses | Complete |
| 4 | [003 repository intelligence](specs/003-repository-intelligence.md) | Lines, languages, directories, Git basics, Cargo deps | In progress |
| 5 | [Engineering guidelines](ENGINEERING.md) | Module tree, the `Analysis` seam, crate lints, MSRV | Planned |
| 6 | [014 project profile](specs/014-project-profile.md) | Purpose and full tech stack, with evidence | Planned |
| 7 | [013 provenance](specs/013-provenance.md) | Origin, fork status, license, authorship | Planned |
| 8 | [015 runbook](specs/015-runbook.md) | Build, run, test, and configure knowledge | Planned |
| 9 | [020 orientation brief](specs/020-brief.md) | **The headline command**, onboard and resume modes | Planned |
| 10 | [004 watch mode](specs/004-watch-mode.md) | Debounced refresh as files change | Planned |
| 11 | [006 local live surface](specs/006-local-web-api.md) | `serve` transport: `TcpListener`, SSE, embedded assets | Planned |
| 12 | [024 view layer](specs/024-view-layer.md) | **Dioxus UI in Rust, compiled to wasm** | Planned |
| 13 | [022 agent activity](specs/022-agent-activity.md) | **Watch an agent build, in real time** | Planned |
| 14 | [026 agentic readiness](specs/026-agentic-readiness.md) | **How is this repo set up for agents**, and configured-versus-used | Planned |
| 15 | [023 forge metadata](specs/023-forge-metadata.md) | Stars, releases, archived state, fork drift | Planned |
| 16 | [008 code annotations](specs/008-code-annotations.md) | TODO harvest and test-surface signals | Planned |
| 17 | [009 symbol index](specs/009-symbol-index.md) | What is defined, and where | Planned |
| 18 | [007 parallel scanning](specs/007-parallel-scanning.md) | Same results, measurably faster | Planned |
| 19 | [011 incremental cache](specs/011-incremental-cache.md) | Warm runs proportional to what changed | Planned |
| 20 | [010 dependency graph](specs/010-dependency-graph.md) | Module and package graphs, cycles, orphans | Planned |
| 21 | [016 subsystem map](specs/016-subsystem-map.md) | Named components a person can hold in mind | Planned |
| 22 | [017 dependency intelligence](specs/017-dependency-intelligence.md) | Versions, staleness, licenses, alternatives | Planned |
| 23 | [021 activity and hotspots](specs/021-activity.md) | Pulse, churn-versus-size risk, knowledge gaps | Planned |
| 24 | [025 practice assessment](specs/025-practice-assessment.md) | **How well is it built** — structure, coupling, test posture, gates | Planned |
| 25 | [018 health assessment](specs/018-health-assessment.md) | Ranked findings with evidence | Planned |
| 26 | [019 visual report](specs/019-visual-report.md) | Self-contained HTML snapshot for sharing | Planned |
| 27 | [012 search index](specs/012-search-index.md) | Interactive-speed content and symbol search | Planned |
| 28 | [005 terminal explorer](specs/005-terminal-explorer.md) | Interactive `ratatui` navigation | Planned |
| 29 | [024 view layer](specs/024-view-layer.md) — desktop | Same component tree in a native WebView shell | Optional |

## Milestones

### Milestone A: Orientation (phases 4-9)

**The product becomes useful to a stranger.** `repo-radar brief` answers what a repository is, what it is built with, how to run it, and where to start reading. Composition stops being a list of file extensions and becomes languages by source bytes, directory weight, and a stated purpose with the file it came from.

### Milestone B: Live (phases 10-12)

One command — `repo-radar serve` — scans, binds loopback, opens a browser, and stays current. The static HTML file stops being the primary visual surface and becomes what it should always have been: an artifact you attach to a review.

The transport and the renderer are separate parcels, because they are separable decisions. Phase 11 hand-rolls HTTP and SSE over `std::net::TcpListener` — no async runtime, no framework — and phase 12 puts the Dioxus view layer on top of the stream it produces.

### Milestone C: The Differentiator (phases 13-15)

Repo Radar reads agent session logs and shows authorship process, live. A developer can watch an agent evolve a project and see which files it is rewriting, which it changed without reading, and which it changed without touching a test.

Agentic readiness then reads the other half: how the repository is *set up* for agents — its subagents, skills, hooks, MCP servers, and whether that work is governed by specs, by plan artifacts, or by conversation alone. Together the two produce the comparison neither can make alone: what a repository claims about its agentic setup against what actually happened in it.

Forge metadata adds the social signal a local clone cannot know.

### Milestone D: Structure (phases 16-21)

Repo Radar understands what is defined, how modules depend on each other, and how the repository decomposes into named subsystems. Performance work lands here, immediately before the analyses expensive enough to need it.

### Milestone E: Judgement (phases 22-25)

Inventory becomes assessment: dependency staleness and licensing, churn-versus-size hotspots, how well the code is built, and ranked health findings that each cite their evidence.

### Milestone F: More Surfaces (phases 26-29)

The shareable snapshot, instant search, the `ratatui` terminal explorer, and — for the cost of a renderer swap over the phase 12 component tree — a native desktop shell.

## Ordering Rationale

- **Safety first.** Phase 3 exists before any feature that reads Git or parses manifests, so every later phase inherits a harness that proves it did not mutate the target. Retrofitting that guarantee is far harder than starting with it.
- **Content before speed.** Parallel scanning was previously scheduled fourth. It has been moved to phase 18. Scanning is not the bottleneck at any repository size this tool has been run against, and optimizing a scan that reports file extensions is optimizing the wrong thing. Performance work now lands with the incremental cache, immediately before the graph and subsystem phases that are genuinely expensive.
- **The brief ships early and grows.** Phase 9 arrives before symbols, graphs, and health exist, because spec 020 requires graceful composition. Shipping it at its thinnest proves the composition rule, and every later phase enriches it automatically.
- **Watch before serve.** The live surface is a consumer of the watch loop. Phase 10 establishes debouncing and the update model; phase 11 streams it.
- **Serve before the static report.** A file written once cannot show a repository that is changing. The snapshot in phase 26 is a narrower artifact and is scheduled as one.
- **Transport before renderer.** Phase 11 is deliberately dependency-free and hand-rolled, because writing HTTP and SSE by hand is both the cheaper parcel and the better lesson. Phase 12 then swaps in Dioxus without touching a line of server code — which is the point of keeping the two separable.
- **The terminal keeps `ratatui`.** Dioxus's terminal renderer is an abandoned alpha; [024](specs/024-view-layer.md) records the evidence. Repo Radar accepts two view layers over one model rather than one view layer over a dead dependency.
- **Agents need something to attribute to.** Phase 13 lands after provenance and the profile so agent events attach to a repository the tool can already describe, and after the view layer so the live pane has a surface to live in.
- **Cache before graph.** Phase 19 precedes the expensive graph and subsystem work so those phases are designed against a warm-run model rather than retrofitted into one.
- **Judgement after structure.** Dependency intelligence, practice assessment, and health all need the symbol index and the dependency graph before their findings can name a component rather than a path.
- **Our own house first.** Phase 5 applies [ENGINEERING.md](ENGINEERING.md) to this codebase before nine more analyses land on top of the current two-file layout. It also fixes the `Analysis` seam once, rather than letting each later analysis invent its own "did this run" convention. Phase 24 then turns those same rules into a feature, with this repository as its first fixture.
- **Interactive surfaces last.** The TUI and the search index are consumers. Building them before the model is stable would fork it.

## Rust Learning Focus per Phase

| Phase | Concepts exercised |
| --- | --- |
| 1-2 | Ownership, borrowing, error handling, `serde` derive, module boundaries |
| 3 | Test harness design, `Drop` guards, property-style invariant assertions |
| 4 | Trait objects, streaming reads, binary detection, table-driven parsing |
| 5 | Module architecture, generic result types, crate-level lints, API stability attributes |
| 6-8 | Encoding detection, table-driven parsing, graceful degradation |
| 9 | Composition over optional data, builder patterns |
| 10 | Channels, debouncing, background threads, clean shutdown |
| 11 | `TcpListener`, thread pools, HTTP and SSE by hand, `include_bytes!` embedding |
| 12 | **Dioxus**: RSX, signals and reactive state, `wasm32` as a build target, cargo workspaces, sharing types across compilation targets |
| 13 | Trait-based adapters, tolerant parsing, tailing an appended file |
| 14 | Versioned detector tables, configuration parsing, credential redaction |
| 15 | Optional network with timeouts, cache TTLs, secret hygiene |
| 16-17 | Line-oriented extraction, versioned static data tables |
| 18-19 | `rayon`, data-parallel iterators, atomic writes, cache invalidation, file locking |
| 20-21 | Arena and index-based graphs, lifetimes without reference cycles, clustering |
| 22-25 | Semantic version comparison, rule tables, threshold models |
| 26 | Deterministic layout, escaping untrusted content, SVG generation |
| 27-29 | `mmap` and unsafe boundaries, `ratatui` state machines, WebView packaging |

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
- Executing any code, task, or command found in a repository, or recorded in an agent log
- Ingesting prompt text or model output from agent sessions
- Cloud synchronization, hosted services, authentication, or telemetry of any kind
- Network access as a default behavior, or transmitting repository content anywhere
- Language-server-level semantic analysis before the filesystem and Git model are stable
- A large frontend before the local core is reusable
- Presenting a heuristic as a fact, or an absent analysis as a passing one
