# Feature Specification: Repository Intelligence

Status: Planned
Priority: P0
Depends on: `001-scan-engine`, `002-structured-output`

## Goal

Move beyond file counting and help a developer decide what deserves attention first.

## Behavior

Add opt-in analysis sections for:

- Line counts for UTF-8 text files, excluding binary files from line totals
- Language families based on a versioned extension mapping
- Largest files and largest directories by aggregate bytes
- Git status counts for modified, untracked, and ignored paths when the root is a Git worktree
- Recent commit activity by day over a configurable window
- A first-pass dependency view for Cargo projects from `Cargo.toml` and `Cargo.lock`

Each analysis reports whether it ran, its result, and a warning when its input is unavailable.

## Acceptance Criteria

1. Binary files do not cause scan failure or inflated text line counts.
2. Directory totals equal the sum of included descendant files.
3. Git analysis never shells out with user-controlled arguments; repository paths are passed safely.
4. A non-Git directory still produces a successful base report.
5. Cargo dependency results distinguish direct dependencies from locked transitive packages where possible.
6. Fixtures cover a Git repository, a non-Git directory, binary content, and a malformed manifest.
7. Each expensive analysis can be disabled from the CLI.

## Constraints

This phase is descriptive only. Repo Radar must not run builds, fetch dependencies, or modify Git state.