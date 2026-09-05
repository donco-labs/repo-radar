# Feature Specification: Dependency Graph

Status: Planned
Priority: P1
Depends on: `003-repository-intelligence`, `009-symbol-index`

## Goal

Build a navigable graph of how a repository holds together, so a developer can see coupling, entry points, and orphaned code.

## Behavior

Construct two related graphs from data already collected:

- A module graph from intra-repository imports (`use` in Rust, `import`/`require` in JavaScript and TypeScript, `import` in Python), where nodes are files or modules and edges are resolved local references.
- A package graph for Cargo workspaces from `Cargo.toml` and `Cargo.lock`, distinguishing direct dependencies from locked transitive packages.

Derived reports:

- In-degree and out-degree per node, with the most-depended-upon files listed first
- Files with no inbound edges, flagged as candidate entry points or dead code
- Detected cycles, reported as the node sequence forming each cycle
- Optional export to Graphviz DOT for external rendering

CLI:

```text
repo-radar graph [PATH] [--scope modules|packages] [--format text|json|dot] [--cycles]
```

## Acceptance Criteria

1. Graph construction uses index-based node identifiers rather than reference cycles, so no `Rc<RefCell<..>>` ownership cycle is required.
2. Imports that resolve outside the repository are recorded as external and never create local nodes.
3. An unresolvable import produces a warning with its path and does not drop the containing node.
4. Cycle detection finds a known planted cycle in a fixture and reports every node in it.
5. A repository with no cycles reports an empty cycle list, not an error.
6. DOT output is parseable by Graphviz and node labels are relative paths.
7. Graph results are deterministic across runs, including node and edge ordering.
8. A fixture covers a Cargo workspace with more than one member crate.

## Constraints

- Resolution is path- and manifest-based. No compilation, no macro expansion, and no network access.
- The graph is a consumer of the scan and symbol data. It must not re-traverse the filesystem.
- Graph size limits must fail loudly with a clear message rather than exhausting memory silently.
