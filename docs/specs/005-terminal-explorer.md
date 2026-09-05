# Feature Specification: Terminal Explorer

Status: Planned
Priority: P1
Depends on: `002-structured-output`, `003-repository-intelligence`, `004-watch-mode`, `009-symbol-index`, `010-dependency-graph`

## Goal

Give developers an interactive view of repository signals without requiring a browser or a long list of flags.

## Behavior

Add an interactive command:

```text
repo-radar tui [PATH]
```

The interface includes:

- A summary header with files, bytes, and warning count
- A sortable file or directory table
- Filters by extension and path substring
- A detail pane for the selected path showing its symbols, annotations, and graph neighbors
- A visible watch/update indicator
- A readable fallback message when stdout is not a terminal
- Keyboard help and a clean `q` exit

## Acceptance Criteria

1. The TUI renders a useful first frame for an empty and a populated fixture.
2. Sorting and filtering update visible rows without changing the underlying scan model.
3. Selection details use the same structured data as text and JSON output.
4. Watch updates do not block keyboard input.
5. Terminal resize does not panic or overlap content.
6. Core state transitions have unit tests independent of terminal rendering.
7. A smoke test documents the supported terminal backend and minimum dimensions.

## Renderer

`ratatui`, deliberately, and not the Dioxus terminal renderer.

[024 view layer](024-view-layer.md) adopts Dioxus for the browser and desktop surfaces, and unifying the terminal under the same component tree was the strongest architectural argument for that choice. It is not available. As checked on 2026-09-05, `dioxus-tui` (formerly `rink`, now `plasmo`) sits at `0.5.0-alpha.0` against a `0.7.10` core — three minor versions behind, never released out of alpha, with its published documentation still pointing at the 0.4 docs.

Repo Radar therefore accepts **two view layers over one model**: Dioxus for browser and desktop, `ratatui` for the terminal. That is a real cost, and it is the cheaper one. Adopting an abandoned alpha to buy architectural elegance would trade a working terminal surface for a speculative one, and this project does not ship speculation.

Revisit if the Dioxus terminal renderer reaches parity with its core version.

## Constraints

The TUI is a consumer of the library. It must not contain filesystem traversal or Git parsing logic.

It shares the JSON contract of [002](002-structured-output.md) with the view layer, not code. Neither surface may define a field the model does not carry, and neither may add analysis of its own.