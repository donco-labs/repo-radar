# Feature Specification: Practice Assessment

Status: Planned
Priority: P1
Depends on: `003-repository-intelligence`, `008-code-annotations`, `014-project-profile`
Deepens with: `009-symbol-index`, `010-dependency-graph`, `016-subsystem-map`

## Goal

Answer "is this codebase built well, and how would I know" — the structural and process questions a reviewer forms in the first hour and usually never writes down.

```text
repo-radar practices [PATH] [--profile NAME] [--config FILE] [--fail-on LEVEL]
```

## What This Is Not

**It is not a linter, and it must never grow into one.** `clippy`, `eslint`, `ruff`, and `go vet` read code far better than Repo Radar will, they run in the project's own toolchain, and duplicating a subset of their rules badly would be worse than useless.

The line is: **a linter looks inside a function; this looks at the shape of the repository.** Whether a `match` is exhaustive is clippy's job. Whether 3,000 lines of analysis sit in one module, whether the public API is four times wider than anything consumes, whether half the source files have no corresponding test, and whether the linter is even wired into CI — none of those are visible to a linter, and all of them decide what the code is like to work in.

It is also not a ranking. There is no comparison against other repositories, no percentile, no grade.

## The Honesty Problem

This feature passes judgement on code the user did not ask us to judge, often code they did not write. It is the most opinionated thing Repo Radar does, and therefore the one most able to discredit it. The requirements in `SPEC.md` apply here at their strictest:

- **No composite score.** No letter grade, no number out of a hundred, no "health percentage". A single figure compresses incomparable dimensions into false precision and invites exactly the ranking this feature refuses. Per-dimension finding counts are permitted; they are counts, not verdicts.
- **Every finding cites evidence** — a file, a line, or a count with the paths that produced it. A finding that cannot point at something does not ship.
- **Every threshold is published with its rationale**, is configurable, and appears in the output alongside the finding it produced. A finding reads `4 files exceed the configured 400-line threshold`, never `poor modularity`.
- **Findings describe, they do not scold.** The output names what is there and what the threshold was. It does not say `bad`, `poor`, `bloated`, or `smelly`.
- **A dimension that cannot be evaluated says so**, with the reason, and is never rendered as a pass. A repository with no CI configuration has *unknown* toolchain gates, not failing ones.
- **Conventions differ, and the tool says so.** A finding is a prompt to look, not a defect. The output states this once, plainly, rather than burying it.

## Dimensions

Language-neutral questions; language-specific detectors. Each dimension reports its findings, the thresholds that produced them, and whether it ran.

| Dimension | Question | Signals |
| --- | --- | --- |
| **Structure** | Is it decomposed into pieces a person can hold? | File and module size distribution, longest files, directory depth and breadth, library/binary split, presence of a module that everything imports |
| **Coupling** | Can one part change without the rest? | Import fan-in and fan-out per module, cyclic imports, public API breadth against internal use, cross-subsystem edges |
| **Test posture** | Would a break be caught? | Test-to-source ratio by bytes and files, source files with no corresponding test, which test layers exist (unit, integration, doc, property, bench), test files that assert nothing |
| **Toolchain gates** | Is quality enforced or hoped for? | Formatter, linter, and test invocation in CI; lint configuration present; declared minimum language version; pinned toolchain; dependency audit step |
| **Error handling** | What happens when input is wrong? | Panic-path density outside tests, presence of a declared error type, error suppression patterns, unchecked results |
| **Safety** | What escapes the language's guarantees? | `unsafe` blocks and their density in Rust; the per-language equivalents elsewhere |
| **Documentation** | Can a stranger start? | Public items with documentation, module-level docs, README sections present, runnable examples |
| **Dependency hygiene** | What has been let in? | Direct count, duplicate versions in the lockfile, floating or wildcard requirements, dependencies with no reference in source |

Coupling's deeper signals need [009](009-symbol-index.md) and [010](010-dependency-graph.md). Until those land, the dimension reports the signals it can derive from imports alone and marks the rest `not evaluated`, per invariant I10.

## Profiles

A **profile** is versioned static data: the detectors, thresholds, and rationales for one language ecosystem. Adding a language adds a profile; it changes no traversal, no aggregation, and no reporting code.

| Profile | Status |
| --- | --- |
| `rust` | Implemented in this phase |
| `typescript`, `python`, `go` | Registered, unimplemented |

A registered-but-unimplemented profile reports `unsupported` with a reason, distinct from "this language is not present here" — the same distinction [022](022-agent-activity.md) draws for agent adapters.

The active profile is selected from the project profile of [014](014-project-profile.md), or forced with `--profile`. A polyglot repository runs every profile whose language is present and reports per profile.

### The Rust profile

Its detectors are the rules in [docs/ENGINEERING.md](../ENGINEERING.md), which is the authoritative statement of them. That document and this profile are maintained together: a rule added there is a detector here, and a detector here without a rule there is a defect.

Defaults, each with the reason it was chosen — thresholds are conventions, not findings of fact, and the output says so:

| Threshold | Default | Rationale |
| --- | --- | --- |
| Module length | 400 lines | Roughly the point past which a file stops being readable in one sitting. Deliberately generous. |
| Function length | 60 lines | A screen. Beyond it, extraction usually clarifies. |
| Public item without docs | any | `#![warn(missing_docs)]` is the ecosystem norm for a library. |
| `unwrap`/`expect` outside tests | any | Each is a panic path; the finding asks for justification, not removal. |
| `unsafe` block | any | Reported always, with location. Presence is not a defect; unexamined presence is. |
| Duplicate lockfile versions | any | A supply-chain and binary-size signal. |
| CI without `fmt`, lint, or test | any | An unenforced gate is not a gate. |

## Behavior

```text
repo-radar practices [PATH] [--profile NAME] [--config FILE] [--fail-on LEVEL]
```

`--config` supplies a threshold file, so a project can encode its own conventions rather than inherit ours. Overridden thresholds are marked as overridden in the output, so a passing result cannot be manufactured invisibly.

`--fail-on LEVEL` exits non-zero when findings at or above a level exist, making the command usable as a CI gate. It is the **only** part of Repo Radar that exits non-zero for a reason other than a usage or path error, and the exit code is documented separately from those.

Findings also feed [018 health assessment](018-health-assessment.md) as one input among several, and the `serve` surface as a panel. This specification owns the analysis; it owns no rendering.

## Dogfooding

**This repository is fixture one.**

A tool that assesses engineering practice and cannot survive its own assessment has disqualified itself. So:

1. `repo-radar practices .` runs in this repository's CI once this phase lands.
2. Its honest output is published in the README, whether or not it flatters us.
3. A finding against Repo Radar is either fixed or recorded in `docs/ENGINEERING.md` as accepted debt with a reason. It is never silenced by loosening a threshold.
4. Our own repository is a permanent test fixture, which means the detectors are exercised against a real, evolving codebase rather than only against synthetic trees.

This is also the honest test of the feature's value: if its findings about our own code are not useful to us, they will not be useful to anyone, and the feature should be cut rather than shipped.

## Acceptance Criteria

1. No output format emits a composite quality score, grade, or percentage; a test asserts the absence of one.
2. Every finding carries a resolvable evidence path, and a line number where the signal is line-anchored.
3. Every finding states the threshold that produced it and whether that threshold was overridden by `--config`.
4. A dimension that cannot run reports `not evaluated` with a reason, distinct from a dimension that ran and found nothing.
5. A registered but unimplemented profile reports `unsupported`, distinct from "language not present".
6. Running against a repository with no CI configuration reports toolchain gates as unknown, never as failing.
7. `--fail-on` exits non-zero only for findings at or above the requested level, and its exit code is distinct from the usage and path error codes.
8. Adding a threshold or a detector to a profile requires no change to traversal, aggregation, or reporting code.
9. The Rust profile's detectors correspond one-to-one with the rules in `docs/ENGINEERING.md`; a test asserts the two lists match.
10. `repo-radar practices .` runs against this repository in CI and its output is reproducible across runs.
11. No finding text contains a pejorative from a documented list (`bad`, `poor`, `bloated`, `smelly`, and similar); a test asserts it.
12. Fixtures cover: a well-structured repository, one exceeding every threshold, one with no tests, one with no CI, one in an unimplemented language, and a polyglot repository.
13. The spec 000 immutability harness passes for every invocation, including `--fail-on` failure paths.

## Constraints

- Assessment is read-only and offline. It reads files and configuration; it never runs a build, a linter, a test suite, or any other command it finds. That would violate invariant I3 and would also mean the assessment could not be trusted on an untrusted clone — which is the whole point.
- Detection is evidence-based. Thresholds are conventions and are labelled as such.
- Profiles are versioned static data. The profile version appears in the output so a changed finding can be attributed to a changed rule rather than to changed code.
- No result is transmitted, cached remotely, compared against other repositories, or retained beyond the invocation.
- Analysis lives here; rendering lives in the surfaces. This specification adds no output format of its own.
