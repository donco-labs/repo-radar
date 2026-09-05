# Build sheets

Historical record of **plan-then-dispatch** work parcels — the complete build sheet the
orchestrating (Opus) model authors *before* a `seam-blaster` (Sonnet) implementation run.

## Why this folder exists

Repo Radar is spec-driven, and that gives it two written layers already: `SPEC.md` says what the
product does, and `docs/specs/NNN-*.md` says what each capability must do and how it is verified.
Neither says *how a particular parcel of work gets built* — which files change, which seam gets
filled, which tests cover which acceptance criteria.

That is the build sheet's job, and in plan-then-dispatch the **plan is the product**: Opus writes
a COMPLETE build sheet → `seam-blaster` implements it verbatim → Opus validates the diff against
the plan, the invariants, and a live smoke → gitify ships it.

Git history keeps the *result* (diff, commit message, PR body). It does **not** keep the *plan*
that produced it. This folder does — so plans can be audited, learned from, and evaluated later
against what actually shipped.

There is a second reason here that STRATUM does not have. Spec 022 makes Repo Radar a tool that
reads agentic authorship process. A repository whose own agentic authorship process is written
down is the honest fixture to develop that against, and eventually to point the finished tool at.

## The workflow

1. Pick the next parcel from [`docs/ROADMAP.md`](../ROADMAP.md).
2. Update the relevant spec first, if behavior is being clarified or added.
3. **Opus authors a complete build sheet** — goal, spec, branch, exact file list with per-file
   change spec, seam contracts, invariant guardrails, test list mapped to the spec's numbered
   acceptance criteria, green-bar commands, gotchas.
4. **Dispatch `seam-blaster`** ([`.claude/agents/seam-blaster.md`](../../.claude/agents/seam-blaster.md)).
   It implements verbatim, runs the green bar, returns a receipt. It does not plan, decide, or
   ship, and it STOPs on any ambiguity, plan-is-wrong, or invariant conflict.
5. **Opus validates** the diff against the plan, the spec 000 invariants, and the acceptance
   criteria — including a **live run of the actual command**, not just a re-run of the green bar.
   A fully green receipt can still be blind to wrong behavior.
6. **gitify** — a separate, separately-authorized step.

Fidelity ≈ plan completeness. A cold Sonnet will not re-derive the spec 000 invariants, the
one-model-many-surfaces rule, or the additive-JSON contract. The build sheet must encode every
invariant the parcel touches, leaving no judgment call. **A thin plan is the failure mode, not
the model.**

## Standing rules

- **Author it at dispatch time**, not after — the build sheet handed to `seam-blaster` *is* the
  artifact. Commit it in the same parcel as the code.
- One file per dispatched parcel, named `YYYY-MM-DD-<slug>-pr<N>.md`.
- Keep the as-authored plan; do not rewrite it to match the result. Divergences between plan and
  shipped code are exactly what is worth reviewing.
- Add a row to the index below per parcel.

## Index

| Date | Parcel | Spec | PR | Sheet |
|---|---|---|---|---|
| 2026-09-05 | Repository intelligence 4a — text signals, directory aggregates | [003](../specs/003-repository-intelligence.md) | _pending_ | [sheet](2026-09-05-repository-intelligence-4a.md) |
