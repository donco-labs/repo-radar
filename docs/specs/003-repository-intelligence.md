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

Line counting and language families are the per-file signals this phase owns. Annotation harvesting is specified in [008](008-code-annotations.md), and the graph view of dependencies is specified in [010](010-dependency-graph.md).

Each analysis reports whether it ran, its result, and a warning when its input is unavailable.

### Delivery in three parcels

This phase is too broad for one change, so it ships as three, each with its own build sheet and green bar:

| Parcel | Scope | Acceptance criteria |
| --- | --- | --- |
| **4a** | Per-file text signals and directory aggregates: line counts, language families, largest directories | 1, 2, 7 |
| **4b** | Git basics: status counts and recent commit activity | 3, 4 |
| **4c** | Cargo dependency view from `Cargo.toml` and `Cargo.lock` | 5 |

Criterion 6's fixtures are added by the parcel that needs each: binary content in 4a, the Git and non-Git directories in 4b, the malformed manifest in 4c.

### Clarifications

**Text versus binary is a heuristic, and is labelled as one.** A file is treated as binary when a NUL byte appears in its first 8 KiB. This is the conventional detection used by Git and `grep`, it is cheap, and it is wrong on rare inputs — a UTF-16 source file reads as binary, and a text file with a stray NUL reads as binary. Full UTF-8 validation of every file was rejected as disproportionately expensive for the accuracy it buys. Binary files contribute their bytes to size totals and their extension to composition, but never a line count.

**Line counting is byte-oriented and streamed.** Lines are `\n` occurrences, plus one when the file is non-empty and does not end in a newline. Files are read through a fixed-size buffer, so memory stays bounded regardless of file size, upholding invariant I9. An empty file is text with zero lines, not binary.

**Directory totals are recursive and include every ancestor.** A file at `src/parser/lex.rs` contributes its bytes to `src/parser`, to `src`, and to the root. Nested directories therefore appear in the same list as their parents, and the list states that its figures are aggregate. The root's total is the natural check on criterion 2 — it must equal the report's byte total — and is excluded from the reported list, where it would be trivially first and say nothing.

**Unmapped extensions are named, not guessed.** The extension-to-language table is versioned static data, and the table version appears in the JSON output. A file whose extension is not in the table is grouped under `[unrecognized]`, so byte totals still reconcile and the gap is visible rather than silently absorbed into a neighbouring language.

**Disabling an analysis reports `not evaluated`, never zero.** `--no-lines` turns off line counting. Every surface must then state that lines were not evaluated. A disabled analysis rendering as `0 lines` would be the exact failure invariant I10 exists to prevent.

## Acceptance Criteria

1. Binary files do not cause scan failure or inflated text line counts.
2. Directory totals equal the sum of included descendant files.
3. Git analysis never shells out with user-controlled arguments; repository paths are passed safely.
4. A non-Git directory still produces a successful base report.
5. Cargo dependency results distinguish direct dependencies from locked transitive packages where possible.
6. Fixtures cover a Git repository, a non-Git directory, binary content, and a malformed manifest.
7. Each expensive analysis can be disabled from the CLI, and a disabled analysis reports `not evaluated` in every output format rather than a zero or an empty result.

## Constraints

This phase is descriptive only. Repo Radar must not run builds, fetch dependencies, or modify Git state.