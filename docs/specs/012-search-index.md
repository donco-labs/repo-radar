# Feature Specification: Search Index

Status: Planned
Priority: P2
Depends on: `009-symbol-index`, `011-incremental-cache`

## Goal

Let a developer find content and definitions across a repository quickly enough to use interactively from the CLI and the terminal explorer.

## Behavior

```text
repo-radar search QUERY [PATH] [--mode literal|symbol] [--glob PATTERN]... [--limit N] [--format text|json]
```

- Literal mode searches file contents and returns path, 1-based line number, and the matching line
- Symbol mode searches the symbol index by name and returns kind, path, and line
- Results are ranked deterministically: exact name matches first, then prefix matches, then remaining matches, then path and line
- Large files are read through memory mapping rather than full buffering
- The index is stored with the incremental cache and reuses its invalidation rules

## Acceptance Criteria

1. A query with no matches exits successfully with an empty result set, not an error.
2. Binary files are excluded from literal results by default.
3. `--limit N` caps results and the report states whether results were truncated.
4. `--glob` restricts the searched set and an invalid pattern is a usage error with exit status `2`.
5. Search results are identical whether the index was warm or cold.
6. Median query latency on a fixture of at least 10,000 files is recorded by a benchmark.
7. Memory-mapped reads do not panic when a file is truncated during the search.

## Constraints

- Start with memory-mapped scanning. A full-text engine such as Tantivy is adopted only if benchmarks justify it, and only behind a Cargo feature flag.
- Query strings are never passed to a shell or to an external process.
- Regular-expression support is a later spec, not part of this phase.
