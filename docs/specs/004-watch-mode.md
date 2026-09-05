# Feature Specification: Watch Mode

Status: Planned
Priority: P1
Depends on: `001-scan-engine`, `002-structured-output`, `003-repository-intelligence`

## Goal

Keep a repository report current while a developer works, without repeatedly rescanning unaffected files.

## Behavior

Add:

```text
repo-radar watch [PATH] [--format text|json]
```

Watch mode observes create, modify, remove, and rename events under the configured root. It debounces bursts, ignores configured directories, updates affected aggregates, and prints a refreshed report. `Ctrl-C` exits cleanly.

## Acceptance Criteria

1. Creating, modifying, and removing a fixture file updates file count, bytes, and extension totals.
2. Events under skipped directories do not trigger report changes.
3. A burst of events produces at most one refresh per debounce window.
4. Rename events do not leave stale paths in the report.
5. Watch mode exits without panic when the root is removed.
6. Tests use a temporary fixture and do not depend on timing tighter than the configured debounce interval.
7. A benchmark records full-rescan versus incremental-update cost.

## Constraints

The event source must be abstracted behind a trait so deterministic tests do not require a live filesystem watcher.