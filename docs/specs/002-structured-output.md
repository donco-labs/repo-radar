# Feature Specification: Structured Output

Status: Planned
Priority: P0
Depends on: `001-scan-engine`

## Goal

Make Repo Radar useful in scripts and establish the stable data contract that future interfaces consume.

## Behavior

Add an explicit output mode:

```text
repo-radar [PATH] --format text|json [--top N]
```

JSON output must contain a version field, repository path, file count, total bytes, extension counts, largest files, and warnings. JSON goes to stdout; diagnostics go to stderr. Text remains the default for humans.

## Acceptance Criteria

1. `--format json` emits one valid JSON document and no human headings.
2. The schema version is present and starts at `1`.
3. Paths in JSON are relative to the scanned root where possible.
4. Warning entries never make the JSON document invalid.
5. `--top 0` produces an empty largest-files array.
6. Integration tests parse the output and assert the required fields.
7. README documents the mode and gives a shell-pipeline example.

## Constraints

The JSON schema is additive within version 1. Removing or renaming a field requires a new schema version and a spec update.