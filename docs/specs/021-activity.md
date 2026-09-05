# Feature Specification: Activity and Hotspots

Status: Planned
Priority: P1
Depends on: `003-repository-intelligence`, `013-provenance`, `016-subsystem-map`

## Goal

Answer "what has been happening here, and where is the risk concentrated" from Git history alone.

## Behavior

```text
repo-radar activity [PATH] [--since DURATION] [--format text|json]
```

### Pulse

- Commits per day and per week over a configurable window, rendered as a sparkline in text output
- Active, slowing, dormant, or abandoned classification from recency and trend, with the thresholds stated in the output
- Contributors active in the window versus all time
- Commit distribution by weekday and hour, which distinguishes a work project from a nights-and-weekends one
- Branch inventory: local and remote branches, their last commit date, and how far each has diverged
- Merge cadence and average time between commits

### Hotspots

Rank files by churn multiplied by size, the classic proxy for where defects concentrate: code that is both large and frequently changed.

- Commit count per file within the window
- Distinct authors per file
- Lines added and removed
- Hotspot score combining churn and size, with the formula documented
- Coupling: files that repeatedly change in the same commit as each other, which reveals hidden dependencies the import graph cannot see
- Stale files: the longest untouched paths, which are candidates for dead code
- Knowledge risk: files whose history has a single author

### Per-subsystem rollup

Every metric above aggregated to the subsystems from spec 016, so a user sees which component is hot, which is abandoned, and which has a single owner.

## Acceptance Criteria

1. `--since` accepts a duration such as `90d` or `12w`, defaults to 90 days, and the window used appears in the output.
2. Rename history is followed so a renamed file's churn is not reset to zero.
3. Merge commits are excluded from churn by default, and the choice is stated and configurable.
4. A repository with a single commit produces a successful report with trends marked as insufficient data.
5. A shallow clone reports that history is truncated and marks derived figures incomplete.
6. Change coupling requires a minimum co-occurrence count before it is reported, and the threshold is stated.
7. The hotspot formula is documented in the output and in the README, and its ranking is reproducible.
8. History is read with fixed argument vectors and optional locks disabled, upholding invariants I2 and I4 of spec 000.
9. Author identities are aggregated locally and never leave the machine.
10. A benchmark records history-analysis cost on a repository with at least 10,000 commits.

## Constraints

- Bus factor, knowledge risk, and abandonment are heuristics about code, not judgements about people, and the output must be worded accordingly.
- History analysis is bounded by the window. An unbounded walk of a large repository is a defect.
- Repo Radar reads history. It never rewrites, amends, or garbage-collects it.
