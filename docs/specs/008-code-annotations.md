# Feature Specification: Code Annotations and Test Signals

Status: Planned
Priority: P1
Depends on: `001-scan-engine`, `002-structured-output`, `007-parallel-scanning`

## Goal

Surface the human markers already present in a repository — unfinished work and test coverage surface area — so a developer can see debt and safety at a glance.

## Behavior

Add an opt-in analysis that reads UTF-8 text files and reports:

- Annotation hits for a configurable marker set defaulting to `TODO`, `FIXME`, `HACK`, `XXX`, and `SAFETY`
- Each hit as marker kind, relative path, 1-based line number, and trimmed line text
- Annotation counts grouped by marker kind
- Test signals: which files are recognized as test files, the ratio of test files to source files, and per-language test file counts

Test file recognition is heuristic and versioned: path segments such as `tests`, `test`, `__tests__`, `spec`, and filename patterns such as `*_test.*`, `*.test.*`, `*_spec.*`, and Rust `#[cfg(test)]` modules.

CLI:

```text
repo-radar [PATH] --annotations [--annotation-marker NAME]... [--max-annotations N]
```

## Acceptance Criteria

1. Binary and non-UTF-8 files are skipped without failing the scan and without producing annotation hits.
2. A marker inside a word (for example `TODOS_LIST`) is not reported; matching is on a word boundary.
3. Line numbers are 1-based and match the source file.
4. `--max-annotations N` caps emitted hits and the report states that results were truncated.
5. Custom markers supplied on the command line replace the default set rather than appending silently, and the chosen set appears in the report.
6. Test-file heuristics are covered by fixtures for Rust, JavaScript, and Python layouts.
7. The analysis is off by default and its cost is not paid when disabled.
8. JSON output gains additive fields only and the schema version stays at `1`.

## Constraints

- No regular-expression engine is required; simple line scanning is acceptable and preferred for auditability.
- Repo Radar reports annotations. It never edits, resolves, or reformats them.
- Coverage percentages from external tooling are out of scope. Only structural test signals derived from the filesystem are reported.
