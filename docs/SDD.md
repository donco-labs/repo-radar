# Spec-Driven Development Policy

Repo Radar uses spec-driven development (SDD). The goal is a repeatable path from intent to verified behavior.

## Authority Order

When sources disagree, use this order:

1. The active product specification in `SPEC.md`
2. A feature specification in `docs/specs/`
3. Tests that encode an accepted requirement
4. Implementation details

If the desired behavior is not specified, stop and clarify the requirement before expanding the implementation.

## Change Loop

Every feature or behavior change follows this sequence:

1. Write or update the authoritative specification.
2. Define observable acceptance criteria and failure behavior.
3. Implement the smallest change that satisfies the criteria.
4. Add or update focused tests.
5. Run formatting, tests, and Clippy with warnings denied.
6. Update user-facing documentation and record the spec in the change description.

## Phase Completion

A roadmap phase closes only when its specification's acceptance criteria are all verified, the specification `Status` reads `Implemented`, the roadmap status column is updated, and `README.md` documents the shipped state.

`README.md` must always describe the tool as it exists on `master`, not as it is planned. At every phase boundary it must contain:

- A **Features** section listing every shipped capability
- A **Getting Started** section with installation steps and runnable examples
- A **Usage** section covering every flag and subcommand the binary accepts
- A **Roadmap** section summarizing what is next, linked to `docs/ROADMAP.md`

A phase that ships behavior without this README update is incomplete, regardless of test status.

## Pull Request Contract

Each change must state:

- Which specification requirement it implements or changes
- What observable behavior changed
- How the acceptance criteria were verified
- Any known limitations or follow-up spec work

Changes that alter behavior without a corresponding specification update are incomplete.

## Quality Gates

The repository must remain clean under:

```text
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

CI is the final repeatable check. Local validation is expected before opening a pull request.