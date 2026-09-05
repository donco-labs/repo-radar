# Feature Specification: View Layer

Status: Planned
Priority: P0
Depends on: `002-structured-output`, `006-local-web-api`

## Goal

One component tree, written in Rust, that renders the Repo Radar model to a browser today and to a native desktop window later, without either surface owning a copy of the model or a copy of the UI.

## Framework Decision

**Dioxus**, compiled to `wasm32-unknown-unknown` for the browser and to a system WebView for desktop.

Recorded because a framework choice is load-bearing and should be auditable, per the evidence rules in `SPEC.md`:

| Fact | Value | Checked |
| --- | --- | --- |
| `dioxus` latest stable | 0.7.10 | 2026-09-05, crates.io index |
| Release cadence | 0.7.6 through 0.7.10 published in the current series | 2026-09-05 |
| `dioxus-web` / `dioxus-desktop` | track the core version | 2026-09-05 |
| `dioxus-cli` (`dx`) | the build tool for the wasm target | 2026-09-05 |

The version is pinned at implementation time and recorded in the parcel's build sheet. Dioxus is pre-1.0 and its API moves between minor versions; this specification describes architecture, not API surface, so a minor bump does not invalidate it.

### Why not the alternatives

- **Leptos** — a close call, and a better fine-grained reactivity story. Dioxus wins on the second target: a desktop shell is a renderer swap over the same component tree, not a second application.
- **Yew** — mature, but momentum and tooling investment have moved elsewhere.
- **Vanilla JS** — rejected. It would make the browser the one surface in this project that is not written in Rust, and would need reimplementing for desktop.
- **egui / iced** — native-only. They do not serve a browser, which is the primary surface in [006](006-local-web-api.md).

### Why this does not cover the terminal

Dioxus has a terminal renderer (`dioxus-tui`, formerly `rink`, now `plasmo`). It is **not adopted here.** As checked on 2026-09-05 it sits at `0.5.0-alpha.0` while the core is at `0.7.10` — three minor versions behind, never released out of alpha, and its published documentation link still points at the 0.4 docs. Unifying the terminal and the browser under one component tree was the strongest architectural argument for Dioxus, and it is currently not available.

[005 terminal explorer](005-terminal-explorer.md) therefore keeps `ratatui`. Repo Radar accepts **two view layers over one model**: Dioxus for browser and desktop, `ratatui` for the terminal. Both are consumers of the JSON contract from [002](002-structured-output.md) and neither may fork it.

If the Dioxus terminal renderer reaches parity with the core version, revisiting spec 005 becomes worthwhile. Until then, adopting an abandoned alpha to chase architectural elegance would trade a working terminal surface for a speculative one.

## Architecture

```text
        ScanReport  (src/lib.rs, the one model)
             │
             ├── serde_json ──> JSON contract, spec 002
             │                        │
             │            ┌───────────┴───────────┐
             │            │                       │
             │      GET /api/v1/report      GET /api/v1/events (SSE)
             │            │                       │
             │            └───────────┬───────────┘
             │                        ▼
             │             ┌──────────────────────┐
             │             │  Dioxus component    │   this spec
             │             │  tree (crate: ui)    │
             │             └──────────┬───────────┘
             │                        │
             │            ┌───────────┴───────────┐
             │            ▼                       ▼
             │      dioxus-web (wasm)      dioxus-desktop (WebView)
             │       served by `serve`        future native shell
             │
             └── ratatui ──> terminal explorer, spec 005
```

### Workspace shape

The crate becomes a workspace. The UI compiles to a different target than the binary and must not drag browser dependencies into the scanner.

| Crate | Target | Depends on |
| --- | --- | --- |
| `repo-radar` | native binary and library | `serde`, `serde_json` |
| `repo-radar-ui` | `wasm32-unknown-unknown` and native WebView | `dioxus`, `repo-radar` (types only, `default-features = false`) |

The UI crate depends on the core crate for its **types**, so the JSON contract is shared as Rust types rather than as a hand-written TypeScript interface that can silently drift. The core crate never depends on the UI crate.

### State model

The client holds one signal per report section, not one signal for the whole report. An SSE frame deserializes once, then writes only the sections whose value actually changed, so a scan where one file grew does not repaint the dependency panel.

```text
Signal<Summary>        files, bytes, warnings count
Signal<Composition>    languages and extensions
Signal<Vec<FileEntry>> largest files
Signal<Vec<Warning>>   scan notes
Signal<Option<Agents>> agent activity, spec 022
Signal<Connection>     Live | Reconnecting | Snapshot
```

`Connection` is not decoration. A surface that claims to be live must show when it has stopped being live, per the honesty requirements in `SPEC.md` — a stale page that looks current is exactly the failure this project refuses elsewhere.

### Build and embedding

`dx build` produces the wasm blob, its JS glue, and an HTML shim. All three are embedded into the native binary with `include_bytes!` / `include_str!` and served by route from memory.

This preserves the property [006](006-local-web-api.md) depends on: there is no asset directory to find at runtime, and **no request path ever becomes a filesystem path**, so the no-traversal claim stays structural rather than a check that could be missed.

## Acceptance Criteria

1. The workspace builds: `cargo build` produces the native binary without pulling `dioxus` into it, asserted by inspecting the binary's dependency tree.
2. `repo-radar serve` runs from a single binary with no adjacent asset files present.
3. The UI crate contains no filesystem traversal, no Git parsing, and no analysis logic. It renders values it is given.
4. The UI crate defines no data field that the spec 002 JSON contract does not already carry.
5. Deserializing an SSE frame updates only the signals whose values changed, verified by a render-count test.
6. A report section the server marks unavailable renders as an explicit gap naming the reason, never as an empty or zero-valued panel.
7. The connection indicator shows `Reconnecting` within one keepalive interval of the stream dropping, and the page states that displayed data is stale.
8. All repository-derived text renders as a text node. A fixture with `<script>` in a filename produces no executable content, asserted in the browser surface as well as the JSON.
9. The page is legible in both light and dark color schemes.
10. Core view state transitions have unit tests that run under `cargo test` on the native target, without a browser or a wasm runtime.
11. The pinned `dioxus` version is recorded in `Cargo.toml` and in the parcel build sheet.
12. The wasm build is a documented, reproducible command, and CI runs it.

## Constraints

- The view layer is a consumer. It contains no analysis logic, defines no model field, and never reads the filesystem.
- Dioxus is pre-1.0. The version is pinned, not floated, and a version bump is its own reviewed parcel with the green bar re-run.
- No CDN, no web font fetch, no external stylesheet, no analytics. The page's only origin is the local binary that served it, and this is asserted by a test that greps the built assets for external URL schemes.
- Adding a UI dependency beyond Dioxus and its renderers requires amending this specification. A component library is not a default.
- The green bar for any parcel touching this crate includes the wasm build, not only `cargo test`.
