# Feature Specification: Project Profile

Status: Planned
Priority: P0
Depends on: `001-scan-engine`, `003-repository-intelligence`

## Goal

Answer "what is this and what is it built with" without the user reading a single file.

## Behavior

### Delivery in three parcels

Phase 6 is too large for one parcel: purpose extraction, the exclusion rules that make a language
ranking honest, and a multi-category detector table are three separable decisions with three
different failure modes.

| Parcel | Scope | Criteria | Status |
| --- | --- | --- | --- |
| **6a** | Project purpose: manifest description, README first paragraph, Git description, each with its evidence path and confidence | 1, 2, 5, 6, 8 | Delivered |
| **6b** | Path classification: vendored, generated, and fixture paths excluded from the language ranking, configurably | 3, 4 | Planned |
| **6c** | The tech stack detector table: runtimes, package managers, frameworks, test and build systems, containers, CI, databases, linting | 1, 2, 7 | Planned |

Criteria 1 and 2 are shared: 6a establishes the evidence-and-confidence seam on a single finding,
and 6c reuses it across every stack category rather than inventing a second shape.

### Purpose

Extract a stated purpose, in priority order, from:

1. A package manifest description field (`Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod` module path, `composer.json`, `*.csproj`)
2. The first substantive paragraph of the README, skipping badges, titles, and images
3. The repository description in Git configuration when present

The result records the value, its source file, and its confidence. Nothing is invented: when no purpose is stated, the report says so.

### Tech Stack

Detect the stack from a versioned detector table, where each detector maps evidence to a finding:

- Languages, ranked by source bytes rather than file count, excluding vendored and generated paths
- Runtimes and their required versions (`rust-toolchain.toml`, `.nvmrc`, `engines`, `python_requires`, `go` directive)
- Package managers, identified by lockfile (`Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `uv.lock`, `poetry.lock`, `go.sum`)
- Frameworks inferred from dependency names (`axum`, `actix`, `react`, `next`, `django`, `fastapi`, `spring-boot`, and similar)
- Test frameworks and their config files
- Build and task systems (`Makefile`, `justfile`, `Taskfile`, `gradle`, `maven`, `cmake`, `bazel`)
- Containerization and orchestration (`Dockerfile`, `compose.yaml`, Kubernetes manifests, `Procfile`)
- CI providers, from `.github/workflows`, `.gitlab-ci.yml`, `.circleci`, and similar
- Databases and infrastructure, from dependency names, migration directories, and compose services
- Linting and formatting configuration

Every finding carries: name, category, version where known, the evidence path that produced it, and a confidence of `certain` (lockfile or manifest) or `inferred` (naming convention or heuristic).

### Clarifications

**The purpose precedence list is a versioned table, not a search.** Priority within the manifest
category is not left to directory order. The table, in order, is: `Cargo.toml` `[package].description`;
`package.json` `.description`; `pyproject.toml` `[project].description` then `[tool.poetry].description`;
`composer.json` `.description`; `go.mod`'s `module` path. Then the README's first substantive
paragraph, then a non-placeholder `.git/description`. A `PURPOSE_TABLE_VERSION` constant travels with
the JSON report the way `LANGUAGE_TABLE_VERSION` already does, so a consumer can tell a table change
from a repository change.

**`*.csproj` is deferred, and the reason is a dependency, not an oversight.** Reading `<Description>`
out of an MSBuild project file means parsing XML, and no XML parser is in the dependency budget for
one optional field. Substring-scanning for the element was considered and declined: comments, CDATA,
attributes, and entity escapes each produce a wrong answer, and a wrong stated purpose is worse than
an absent one under this specification's own evidence rule. Revisit when another feature needs XML.

**The Git description is read from `.git/description`, as a file.** It is the only place a local clone
records a description, and reading the file needs no subprocess. Git's own placeholder
(`Unnamed repository; edit this file 'description' to name the repository.`) is not a stated purpose
and is skipped. When `.git` is a file rather than a directory — a worktree or a submodule — the source
is simply unavailable. Confidence is `inferred`: criterion 2 reserves `certain` for manifests and
lockfiles.

**A statement is normalized, then capped at 500 characters.** Every source's raw text has its
whitespace runs collapsed to a single space and is trimmed, so a TOML multi-line description and a
wrapped README paragraph produce the same shape. The cap cuts on a character boundary, prefers the
last space before it, and sets a `truncated` flag — a truncated statement is never presented as a
complete one. Normalizing is not sanitizing: the statement is untrusted repository content and is
made safe by the renderer for its medium, never at extraction time.

**A malformed manifest degrades to the next source and warns.** It does not abort the profile
(criterion 6) and it does not fail the scan. The warning names the file and is tool-authored: parser
diagnostic text quotes repository content back and is never reported, the rule spec 003 established
for both git's stderr and `toml::de::Error`.

**Purpose undetermined is `not evaluated`, not an empty string.** A repository with no manifest
description, no README prose, and no Git description reports `InputUnavailable` with the sources it
looked for — invariant I10, rather than a blank statement a surface could render as a fact.

**A README's skip rules must treat a Setext heading as two lines, and front matter as metadata.**
Both were found during 6a's validation, by a live run, against a fully green 119-test bar — the
build sheet's own rules were incomplete and its test 6 used an ATX heading, so neither gap could
fail a test. A Setext heading is a title line *plus* an `===` or `---` underline; skipping only the
underline leaves the title standing as the first substantive line, which is the "titles" case
criterion 8 exists to skip. A paragraph must likewise end *before* the title of a Setext heading
that follows it. YAML (`---`) and TOML (`+++`) front matter is a static-site generator's metadata,
and its first key reads as a plausible sentence (`title: Docs Site`) while being nothing of the
kind; it is stripped when it appears at the very top of the file *and closes*, so that a README
merely opening with a horizontal rule keeps all of its content.

The general lesson is the one this project keeps relearning: **a green bar proves the tests pass,
not that the behavior is right.** Prose has no grammar, so its rules cannot be derived from a
format specification, and every one of them needs a live run against a real-shaped document.

## Acceptance Criteria

1. Every stack finding names the evidence file that produced it; a finding with no evidence path is a defect.
2. Confidence is `certain` only when it derives from a manifest or lockfile entry.
3. A polyglot fixture with Rust, TypeScript, and Python reports all three languages ranked by source bytes.
4. Vendored, generated, and test-fixture paths are excluded from language ranking, and the exclusion rules are configurable.
5. A repository with no manifest and no README produces a successful report stating that purpose is undetermined.
6. A malformed manifest produces a warning and does not abort the profile.
7. The detector table is versioned data, and adding a framework requires no change to traversal or reporting code.
8. README purpose extraction skips badge lines, headings, and HTML comments, and is capped at a documented length.

## Constraints

- Detection is evidence-based. Repo Radar reports what it found and where, never a guess presented as fact.
- No dependency is resolved over the network in this phase.
- The detector table's accuracy limits are documented in the README rather than implied to be complete.
