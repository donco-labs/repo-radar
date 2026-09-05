# Feature Specification: Orientation Brief

Status: Planned
Priority: P0
Depends on: `013-provenance`, `014-project-profile`, `015-runbook`

## Goal

One command that tells a developer what they need to know to start working, in under a page. This is the feature the rest of the tool exists to serve.

Two audiences, both arriving without context:

- **Newcomer**: cloned this from elsewhere and has never read it
- **Returning author**: wrote this, and has forgotten it

The difference between them is not what the repository contains. It is what has happened since they last looked.

## Behavior

```text
repo-radar brief [PATH] [--mode auto|onboard|resume] [--since DURATION] [--format text|json|markdown]
```

Mode `auto` selects `resume` when the user has local authorship in the repository's history or the working tree is dirty, and `onboard` otherwise. The chosen mode and the reason for it are stated in the output.

### Onboard mode

1. **What this is** — purpose, and where it came from
2. **Stack** — languages, frameworks, and runtime versions required
3. **Shape** — subsystems and their relationships, as a text diagram
4. **How to run it** — the quick-start from spec 015
5. **State of health** — top findings, with the worst first
6. **Where to start reading** — entry points, then the highest in-degree files, because they are what everything else depends on
7. **What to be careful with** — hotspots, cycles, and single-author subsystems

### Resume mode

Answers "where was I", assuming the repository is already understood:

1. **Since you last looked** — commits, contributors, and changed subsystems since the last commit authored by the current user, or since `--since`
2. **Uncommitted work** — working tree changes, grouped by subsystem, with the annotations they contain
3. **Your branches** — local branches with unmerged work, their age, and their divergence
4. **Loose ends** — annotations in files the user touched most recently, newest first
5. **What changed underneath you** — files you have authored that others have since modified
6. **New arrivals** — dependencies added and subsystems created since the window began
7. **Health delta** — findings that appeared since the window began

### Graceful composition

The brief renders whatever analyses are available. A section whose input is not yet implemented, or failed, is omitted from the body and listed under `not available` with its reason. The brief must therefore be shippable before its dependencies are, and improve as they land.

## Acceptance Criteria

1. The default text brief fits in 80 columns and is capped at a documented length, prioritizing by severity and recency when content exceeds the cap.
2. Every claim in the brief is traceable to an evidence path, and `--format json` exposes that evidence.
3. `auto` mode selection is deterministic given a repository state and current user identity, and the selection reason is always stated.
4. `resume` mode on a repository with no local authorship degrades to `onboard` with a stated reason rather than producing an empty brief.
5. A repository with no Git history, no manifest, and no README still produces a useful brief describing what was found.
6. Sections whose inputs are unimplemented are listed as unavailable, never silently skipped and never fabricated.
7. Markdown output is valid and safe to paste into an issue, with repository content escaped.
8. The brief states the tool version, scan time, and the commit it describes.
9. A fixture-based test asserts the brief is unchanged across two consecutive runs of an unchanged repository.
10. Total runtime on a 5,000-file repository is benchmarked and stays within a documented budget on a warm cache.

## Constraints

- The brief summarizes the model. It contains no analysis logic of its own, so a fact can never differ between `brief` and the command that owns it.
- Nothing is invented. Absent input produces an explicit gap, never a plausible-sounding guess.
- Length discipline is the feature. A brief that becomes a full report has failed, and the cap is a requirement rather than a default.
