---
name: gitify
description: "Full ship flow for a finished work parcel: branch → green bar → commit → push → open PR into master → wait on CI → merge. Trigger when the user says 'gitify', 'ship it', or asks to commit+push+PR+merge as one action."
---

# gitify — one-word ship pipeline

`gitify` takes the current finished work parcel all the way to a merged pull request. The user saying `gitify` is the commit and push authorization; run the flow without pausing between steps.

## How to run it

Delegate the ship flow to the `gitify` subagent when one is available. Pass a short summary of what this parcel changed, why it changed, and how it was verified so the commit message and PR body retain the rationale.

The runner owns the full procedure: branch-per-parcel, green checks, Conventional Commit with the Co-Authored-By trailer, push, PR creation, CI wait, squash merge, and local branch synchronization.

## Do it inline instead when

- The parcel is mid-flight and the current agent already holds the green-bar and commit context.
- The user explicitly says to ship without delegating.

## Procedure

1. **Branch.** Each parcel uses a `feat/`, `fix/`, `docs/`, or `chore/` branch. If currently on `master`, create one first and carry the work over.
2. **Green bar.** Run the checks for the touched components. For this Rust project, run the commands in `docs/SDD.md`.
3. **Commit.** Use a Conventional Commit with a subject of roughly 50 characters or fewer. End the message with:

   ```text
   Co-Authored-By: Claude <noreply@anthropic.com>
   ```

4. **Push.** Run `git push -u origin <branch>`.
5. **PR.** Open a pull request into `master` with a real what/why/verification body. End the body with:

   ```text
   🤖 Generated with [Claude Code](https://claude.com/claude-code)
   ```

6. **Wait on CI.** Use `gh pr checks <pr> --watch`. Never merge while checks are pending or red. If no CI is configured, record that and continue.
7. **Merge.** Run `gh pr merge --squash --delete-branch`, then synchronize local `master`.

## Stop conditions

- Green checks fail: fix only if the cause is trivial; otherwise stop and report.
- CI is red: stop and report; do not merge.
- Behavior changed without a matching specification update: stop under the SDD policy.
- A decision requires the user: stop and ask.

This skill is the standing definition of the Repo Radar `gitify` shorthand.