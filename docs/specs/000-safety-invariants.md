# Feature Specification: Safety Invariants

Status: Implemented
Priority: P0
Depends on: nothing. Every other specification depends on this one.

## Goal

Repo Radar must always be safe to run against any repository, including one the user does not trust and has not read. The target of an inspection is immutable. This document is the constitution: where any other specification appears to permit a violation, this one wins.

## Invariants

### I1. The target is read-only

Repo Radar must never create, modify, delete, rename, truncate, or change the permissions of any path inside the scanned root. This holds for every command, every flag, and every failure path, including panics and interrupts.

### I2. Git state is never mutated

No `fetch`, `pull`, `push`, `checkout`, `commit`, `stash`, `gc`, `prune`, or index refresh. Read-only plumbing only. Git must be invoked with optional locks disabled so that a read cannot take a lock or rewrite the index as a side effect.

### I3. Repository content is never executed

Repo Radar does not run build scripts, task definitions, hooks, tests, containers, or any command it extracted from the repository. Extracted commands are inert text. This is permanent, not a phase constraint: executing code from an unfamiliar clone is the exact risk this tool exists to reduce.

### I4. Repository content is untrusted input

File contents, paths, branch names, author names, manifest fields, and commit messages are attacker-controlled data. They are never interpolated into a shell command, never used to construct a filesystem path outside the root, and never passed as flags to a subprocess. Terminal control sequences are stripped before any content reaches a terminal.

### I5. Writes go outside the target, and only where asked

Caches and indexes live in the platform cache directory, keyed by root. Report files are written only to a path the user named on the command line. Repo Radar never writes into the scanned repository even when the user points an output path there; that case is a usage error.

### I6. Offline by default

No command performs network access unless the user passes an explicit opt-in flag. Network access is never implied by another flag, never enabled by a configuration file, and never retried into existence. When a network operation is opted into, it may transmit only public package coordinates, never repository content, paths, names, or metrics.

### I7. No telemetry, ever

Repo Radar collects no usage data, sends no analytics, and reports no crash data. This is not configurable, because a configurable version of this promise is not a promise.

### I8. Traversal stays inside the root

Symbolic links are not followed. Path traversal in manifests, configuration, or overrides cannot escape the scanned root.

### I9. Hostile input degrades, it does not crash

Deeply nested trees, enormous files, cyclic structures, invalid UTF-8, and malformed manifests produce warnings and bounded resource use, never a panic, an unbounded allocation, or a descriptor leak.

### I10. Failure is loud

A check that cannot run reports `not evaluated`. Repo Radar never reports absence of evidence as evidence of health, and never silently substitutes a default for input it failed to understand.

## Acceptance Criteria

1. A shared test harness records a recursive digest of a fixture tree — paths, sizes, contents, modification times, and permissions — before and after every command, and asserts the digest is unchanged. Every command-level integration test uses it.
2. The harness covers the failure paths as well as the success paths, including a missing root, a malformed manifest, an unreadable file, and an interrupted run.
3. A test asserts the fixture's `.git` directory is byte-identical after every Git-reading operation.
4. A test asserts that a default run of every command performs no network access, enforced by a mechanism that fails the test on any outbound connection attempt.
5. A test asserts that a run against a fixture containing a hostile filename, a hostile branch name, and a hostile manifest field produces no shell interpolation and no escape from the root.
6. A test asserts terminal control sequences present in repository content do not reach terminal output intact.
7. An output path inside the scanned root exits with a usage error rather than writing.
8. Every specification that adds a command states how it upholds these invariants, or names the invariant it needs relaxed and why.

## Enforcement Status

The harness lives in `tests/common/mod.rs` and the invariant tests in `tests/safety_invariants.rs`. This section records the enforcement level honestly, because a specification that overstates its own coverage is the same defect it exists to prevent.

| Criterion | Status | Notes |
| --- | --- | --- |
| 1. Digest before and after every command | Enforced | Covers contents, sizes, modification times, permissions, and symlink targets, across every current invocation |
| 2. Failure paths covered | Enforced | Missing root, bad flag value, unknown flag, extra argument, unreadable directory, malformed manifest |
| 3. `.git` byte-identical | Enforced | Plus a check that `HEAD` and `git status --porcelain` are unchanged |
| 4. No network on a default run | Partial | Enforced at the dependency level: no network-capable crate may enter `Cargo.lock`. A syscall-level guard is required when spec 017 adds `--online` |
| 5. No shell interpolation, no escape from the root | Enforced | Ten hostile file names including command substitution, separators, and flag-shaped names, with a canary file outside the root |
| 6. Control sequences do not reach output | Enforced | Fixed a real defect: file names reached the terminal with escape sequences intact |
| 7. Output path inside the root is a usage error | Deferred | No command accepts an output path yet. A test pins the stronger current property — Repo Radar writes nothing at all — so specs 011 and 019 must extend it rather than quietly gaining a write |
| 8. Every spec states how it upholds these invariants | Ongoing | Applies to each feature spec as it is implemented |

Two criteria are not fully met. Criterion 4 cannot observe a syscall without a sandbox, so it guards the property that makes a syscall possible. Criterion 7 has nothing to test until a command can write. Both are recorded here rather than marked complete.

The harness is also tested against itself: one test asserts it detects created, modified, and removed files and created directories, and another asserts that running it over an empty tree fails rather than trivially passing. An immutability harness that cannot fail would make every test built on it worthless.

## Constraints

- These invariants are testable claims, not aspirations. An invariant with no enforcing test is a defect in this specification.
- Relaxing an invariant requires changing this document first, in its own reviewed change, separate from the feature that wants the relaxation.
