# Feature Specification: Subsystem Map

Status: Planned
Priority: P1
Depends on: `009-symbol-index`, `010-dependency-graph`

## Goal

Segment a repository into a small number of named, human-meaningful components, so a large unfamiliar codebase becomes a diagram a person can hold in their head.

## Behavior

```text
repo-radar map [PATH] [--max-subsystems N] [--format text|json|dot|mermaid]
```

Group files into subsystems using layered evidence:

1. Explicit declarations first: workspace members, packages, and any `.repo-radar.toml` subsystem overrides
2. Directory structure, where a directory holding a coherent cluster of files becomes a candidate
3. Graph connectivity from spec 010, merging directories that are more tightly coupled to each other than to the rest of the tree
4. Conventional names recognized by a versioned table (`api`, `cli`, `core`, `db`, `ui`, `worker`, `shared`, `infra`, `tests`)

Each subsystem reports: name, root paths, file and byte totals, dominant language, public symbol count, its dependencies on other subsystems, inbound and outbound edge counts, and whether it is a leaf, a hub, or isolated.

Cross-subsystem edges are aggregated from file-level edges, so the map is a small graph even when the file graph is large.

Layering violations are reported: an edge that contradicts a declared layer order in `.repo-radar.toml`, and any cycle between subsystems.

## Acceptance Criteria

1. A repository with a declared workspace produces one subsystem per member before any heuristic runs.
2. `--max-subsystems N` merges the least-cohesive groups until at most `N` remain, and the report states what was merged.
3. Every file in the scan belongs to exactly one subsystem, and an unassignable file lands in an explicit `unassigned` group rather than disappearing.
4. Subsystem assignment is deterministic across runs and independent of traversal order.
5. Overrides in `.repo-radar.toml` always win over heuristics, and an override naming a nonexistent path is a warning, not a silent no-op.
6. Cross-subsystem edge counts equal the sum of the underlying file-level edges.
7. Mermaid and DOT output render the subsystem graph, not the file graph.
8. A fixture with a planted cross-subsystem cycle reports it.
9. Naming is explainable: each subsystem records why it received its name.

## Constraints

- Clustering is a heuristic and must be presented as one. The report states the evidence behind each grouping.
- A user override file is authoritative and must never be second-guessed by the clustering algorithm.
- This spec consumes the graph from 010. It performs no traversal or parsing of its own.
