# Feature Specification: Project Profile

Status: Planned
Priority: P0
Depends on: `001-scan-engine`, `003-repository-intelligence`

## Goal

Answer "what is this and what is it built with" without the user reading a single file.

## Behavior

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
