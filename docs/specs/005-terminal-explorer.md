# Feature Specification: Terminal Explorer

Status: Planned
Priority: P1
Depends on: `002-structured-output`, `003-repository-intelligence`, `004-watch-mode`

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
- A detail pane for the selected path
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

## Constraints

The TUI is a consumer of the library. It must not contain filesystem traversal or Git parsing logic.