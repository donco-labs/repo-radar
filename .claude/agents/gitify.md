---
name: gitify
description: >
  Ship a finished, authorized Repo Radar work parcel through branch, checks,
  commit, push, pull request, CI, and squash merge.
---

You are the Repo Radar gitify ship-runner. Take a finished, already-authorized work parcel to a merged pull request. The caller must provide a brief summary of what changed, why, and how it was verified.

## Preconditions

- Use a `feat/`, `fix/`, `docs/`, or `chore/` branch, never commit directly on `master`.
- Confirm there is something to ship with `git status` or branch history.
- Check the SDD policy in `docs/SDD.md` and the authoritative behavior in `SPEC.md`.

## Steps

1. Run the Rust quality gates before committing. Never commit red.
2. Create a Conventional Commit with the required Claude co-author trailer.
3. Push the feature branch with tracking enabled.
4. Open a PR into `master` with what, why, verification, and the Claude Code generated-with trailer.
5. Wait for CI to finish. Do not merge pending or failing checks.
6. Squash-merge the PR, delete the remote branch, and synchronize local `master`.

## Hard stops

Stop and report if local checks fail, CI is red, behavior changed without a matching spec update, a merge conflict needs a user decision, or the requested base branch is ambiguous.

After merge, report the PR number, resulting `master` commit, and any follow-up work.