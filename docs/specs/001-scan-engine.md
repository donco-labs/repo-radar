# Feature Specification: Scan Engine

Status: Implemented
Priority: P0
Depends on: `SPEC.md`

## Goal

Extract repository scanning from the binary into a reusable library with deterministic traversal, explicit configuration, and testable failure behavior.

## Behavior

- Scan regular files recursively from a requested root.
- Never follow symbolic links.
- Skip `.git`, `target`, and `node_modules` by default.
- Accept additional ignored directory names through a scan configuration.
- Return relative paths, byte sizes, and normalized extensions.
- Continue scanning when an individual entry cannot be read, while returning a structured warning.
- Return a fatal error when the root is missing or is not a directory.
- Produce deterministic results independent of filesystem directory ordering.

## Acceptance Criteria

1. The CLI output remains behaviorally compatible with `SPEC.md`.
2. The scanner can be called from Rust tests without spawning a process.
3. Two scans of the same fixture produce equal file and extension results.
4. A permission or entry-read problem is represented as a warning with its path.
5. Unit tests cover skipped directories, symlinks, empty directories, missing roots, and extension normalization.
6. A benchmark fixture establishes a baseline for later parallelization.

## Constraints

The engine must remain synchronous in this phase. Concurrency and watch events are separate features so their costs can be measured independently.