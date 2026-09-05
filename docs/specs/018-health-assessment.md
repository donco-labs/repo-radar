# Feature Specification: Health Assessment

Status: Planned
Priority: P1
Depends on: `008-code-annotations`, `010-dependency-graph`, `016-subsystem-map`, `017-dependency-intelligence`, `021-activity`

## Goal

Answer "what is wrong with this repository and what should I look at first", as a ranked list of findings a developer can act on.

## Behavior

```text
repo-radar health [PATH] [--min-severity LEVEL] [--format text|json] [--fail-on LEVEL]
```

Evaluate table-driven checks across dimensions already collected:

- **Testing**: test-to-source ratio, subsystems with no test files, source files with no corresponding test
- **Documentation**: missing README, missing license, undocumented public symbols, stale README referencing paths that no longer exist
- **Debt**: annotation density per subsystem, annotations older than a configurable age from Git blame
- **Structure**: dependency cycles, files with outsized in-degree, orphaned files, subsystems exceeding a size threshold
- **Dependencies**: severity inherited from spec 017 for staleness, license conflicts, duplicates, and unused entries
- **Activity**: abandoned subsystems, single-author subsystems, and churn hotspots from spec 021
- **Hygiene**: committed secrets patterns, committed build artifacts, oversized files in Git history, missing CI configuration

Each finding carries: a stable identifier, severity (`critical`, `high`, `medium`, `low`, `info`), the subsystem and paths involved, the evidence, the reason it matters, and a suggested next action.

A headline score per dimension is reported alongside the raw counts that produced it, never instead of them.

## Acceptance Criteria

1. Every finding carries a stable identifier so results are diffable across runs and suppressible by identifier.
2. Every finding states the evidence that produced it; a finding without evidence is a defect.
3. Suppressions in `.repo-radar.toml` are honored and the report states how many findings were suppressed.
4. Scores are reproducible: the same repository state yields the same score, and the weighting table is data, not code.
5. A healthy fixture reports zero findings above `info` and does not manufacture problems to fill a report.
6. `--fail-on LEVEL` makes the command usable as a CI gate.
7. Secret detection reports the file and line but never the matched value.
8. Every check can be disabled individually.
9. A repository missing an input analysis reports those checks as `not evaluated`, never as passing.

## Constraints

- A check that cannot explain itself does not ship. Every finding must be traceable to evidence a user can verify by hand.
- The score is a summary of findings, never a substitute for them, and the README must state its limits.
- Repo Radar reports. It never edits code, rewrites manifests, or opens pull requests.
