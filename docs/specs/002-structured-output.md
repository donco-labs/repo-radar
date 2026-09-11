# Feature Specification: Structured Output

Status: Implemented
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

### Reporting an analysis that did not run

Invariant I10 requires that an analysis which could not run says so rather than reporting a
plausible default. In JSON this is carried by three fields on the analysis object:

| Field | Presence | Meaning |
| --- | --- | --- |
| `evaluated` | Always | `true` if the analysis ran, `false` otherwise |
| `reason` | Only when `evaluated` is `false` | Why it did not run: `disabled`, `input_unavailable`, `unsupported`, or `failed` |
| `detail` | Only when the reason carries one | Free text naming the specific input, e.g. the manifest that would not parse |

The analysis object keeps its full field shape whether or not it ran, so a consumer's field
access never fails. Those fields are zero-valued when `evaluated` is `false`, and `evaluated`
is what says the zeros are not measurements. `reason` distinguishes the four cases a consumer
needs told apart: you switched it off, there is nothing here to read, we cannot read this kind
of input yet, and we tried and failed.

`lines` is the first analysis to carry this shape. Every analysis added from spec 003 onward
uses it.

## Acceptance Criteria

1. `--format json` emits one valid JSON document and no human headings.
2. The schema version is present and starts at `1`.
3. Paths in JSON are relative to the scanned root where possible.
4. Warning entries never make the JSON document invalid.
5. `--top 0` produces an empty largest-files array.
6. Integration tests parse the output and assert the required fields.
7. README documents the mode and gives a shell-pipeline example.
8. An analysis that did not run emits `evaluated: false` and a `reason`, and still emits its own fields at their zero values.
9. An analysis that ran emits `evaluated: true` and emits neither `reason` nor `detail`.

## Constraints

The JSON schema is additive within version 1. Removing or renaming a field requires a new schema version and a spec update.

`reason` and `detail` are additive: they appear alongside the existing `evaluated` flag and the
analysis's own fields, none of which change name, position, or value. The schema stays at
version 1.