# Feature Specification: Provenance and Identity

Status: Planned
Priority: P0
Depends on: `001-scan-engine`, `002-structured-output`

## Goal

Answer "where did this code come from and whose is it" for a repository the user did not write. This is the first question when opening a clone.

## Behavior

Report identity facts derived from the Git directory and the working tree:

- Origin remote URL, normalized to host, owner, and repository name, with credentials stripped
- All configured remotes and their fetch URLs
- Fork evidence: presence of an `upstream` remote, and its divergence from `origin`
- Current branch, the default branch, and ahead/behind counts against the tracked remote
- Working tree state: clean, or counts of modified, staged, and untracked paths
- First commit date, last commit date, and repository age
- Approximate local clone date from the `.git` directory creation time, reported as approximate
- Commit count and distinct author count
- Top authors by commit count, with a bus-factor estimate: the smallest number of authors accounting for half of all commits
- Declared license from `LICENSE`, `LICENCE`, `COPYING`, or a manifest field, resolved to an SPDX identifier where recognized
- Governance files present: `CODEOWNERS`, `CONTRIBUTING`, `SECURITY`, `CHANGELOG`, `CODE_OF_CONDUCT`
- Vendored or third-party directories detected in the tree
- Whether the root is a monorepo or multi-package workspace

## Acceptance Criteria

1. A directory that is not a Git worktree produces a successful report with provenance marked unavailable and a reason.
2. Remote URLs containing embedded credentials have them removed before the URL reaches any output.
3. SSH (`git@host:owner/repo.git`) and HTTPS remote forms both normalize to the same host, owner, and repository triple.
4. A detached `HEAD` is reported as detached rather than as a branch named `HEAD`.
5. A shallow clone is detected and reported, and commit-count-derived figures are marked as incomplete.
6. Clone date is labelled approximate and is omitted rather than guessed when the timestamp is unavailable.
7. License detection resolves at least MIT, Apache-2.0, BSD-3-Clause, GPL-3.0, AGPL-3.0, and MPL-2.0 from file content, and reports `unknown` rather than guessing.
8. Bus factor is documented as a heuristic, and a single-author repository reports `1`.
9. Fixtures cover a non-Git directory, a shallow clone, a detached head, a fork with an upstream remote, and a repository with no license file.

## Constraints

- Git data is read through a library or by invoking Git with fixed argument vectors. User-controlled values are never interpolated into a shell string.
- Repo Radar never fetches, pulls, pushes, or otherwise mutates Git state.
- Author names and email addresses are repository content. They are reported locally and never transmitted anywhere.
