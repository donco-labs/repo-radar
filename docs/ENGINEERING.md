# Repo Radar Engineering Guidelines

Status: Active
Updated: 2026-09-05

`SPEC.md` says what Repo Radar does. `docs/specs/` says what each capability must do and how it is verified. This document says **how the code itself is built** — the Rust idiom, module structure, and coupling rules every parcel is held to.

It is deliberately short on principle and long on mechanism. A guideline nobody can check is a guideline nobody follows, so each rule below states whether it is **enforced**, **committed**, or **aspirational**, and the project does not pretend a rule is holding when it is not. That honesty requirement is the same one `SPEC.md` applies to analysis results, turned on ourselves.

The intended end state is that most of this file is machine-checked by [025 practice assessment](specs/025-practice-assessment.md) — the feature that assesses these same concerns for any repository, with this one as its first fixture.

## Status of each rule

| Rule | Status | Checked by |
| --- | --- | --- |
| `cargo fmt` clean | **Enforced** | green bar + CI |
| `clippy -D warnings`, all targets, all features | **Enforced** | green bar + CI |
| Tests pass | **Enforced** | green bar + CI |
| Target repository is never modified | **Enforced** | `tests/safety_invariants.rs` |
| No `unsafe` | **Committed** | needs `#![forbid(unsafe_code)]` |
| MSRV declared | **Committed** | needs `rust-version` in `Cargo.toml` |
| Public API documented | **Committed** | needs `#![warn(missing_docs)]` |
| No `unwrap`/`expect` outside tests | **Committed** | needs a clippy lint entry |
| Module size and coupling limits | **Aspirational** | 025, once it exists |
| Public API surface stays minimal | **Aspirational** | 025, once it exists |

## Architecture

### The rule

**The library is the product. The binary is a shell.**

`src/lib.rs` and its modules hold every piece of analysis. `src/main.rs` parses arguments and dispatches. Every surface — text, JSON, HTML, `serve`, the view layer, the TUI — is a *consumer* of the model and contains no analysis of its own. This is already stated in `SPEC.md` as a product rule; here it is a code rule with a structural consequence: **if a surface needs a fact, the fact belongs in the library, not in the surface.**

### Target module tree

The crate currently keeps traversal, sanitizing, and rendering in two large files. That is fine at 700 lines and will not be fine at 3,000, which is where the roadmap takes it. The target:

```text
src/
  lib.rs            Public API surface, re-exports, the report model. Thin.
  scan/
    mod.rs          Traversal, ignore rules, symlink policy
    warnings.rs     Non-fatal error collection
  analysis/
    mod.rs          The Analysis seam (below)
    lines.rs        Line counts, text-versus-binary
    languages.rs    Versioned extension table
    git.rs          Provenance and activity            (parcel 4b)
    cargo.rs        Manifest and lockfile              (parcel 4c)
    agents.rs       Agent adapters                     (spec 022)
    forge.rs        Forge metadata                     (spec 023)
  render/
    text.rs         Human summary
    json.rs         The versioned contract
    html.rs         The static snapshot
  sanitize.rs       Terminal and HTML escaping of untrusted content
  main.rs           Argument parsing and dispatch. Nothing else.
```

Two coupling rules fall out of it, and both are load-bearing:

1. **`analysis/*` modules never know about `render/*`.** An analysis produces data. How it looks is not its concern, and an analysis that formats a string for display has leaked a presentation decision into the model.
2. **`render/*` modules never read the filesystem, run Git, or open a socket.** A renderer that can perform I/O against the target is a renderer that can violate invariant I1, and the whole immutability argument gets harder to make.

**When to do this refactor:** as its own parcel, no behavior change, immediately after parcel 4a lands and before 4b. Doing it now would collide with work in flight; doing it after 4b and 4c means moving three times as much code and rewriting two build sheets' worth of file paths.

### The `Analysis` seam

Every analysis from spec 003 onward has the same shape: it either ran or it did not, it names its evidence, and it can be switched off. Spec 003's honesty rule and invariant I10 both demand that "did not run" is never rendered as a zero.

Encode that in the type system rather than in a convention each analysis re-invents:

```rust
/// The result of an analysis that may not have been able to run.
///
/// Invariant I10 requires that an analysis which did not run reports
/// `not evaluated` rather than a plausible default. Making that a type
/// rather than a convention means a caller cannot render a zero where it
/// meant "unknown" — there is no zero to reach for.
pub enum Analysis<T> {
    Ran(T),
    NotEvaluated(NotEvaluated),
}

pub enum NotEvaluated {
    /// Switched off for this invocation, e.g. `--no-lines`.
    Disabled,
    /// The input does not exist here, e.g. not a Git worktree.
    InputUnavailable(String),
    /// Recognized but not implemented, e.g. an unimplemented agent adapter.
    Unsupported(String),
    /// The input existed and could not be understood.
    Failed(String),
}
```

Every surface then handles `NotEvaluated` explicitly because the compiler makes it, and the four reasons are distinguishable — "you turned it off", "there is no Git here", "we cannot read Cursor logs yet", and "the manifest is malformed" are four different things a user needs told apart.

**Known debt:** parcel 4a ships `LineCounts { evaluated: bool, … }`, which is the weaker version of this — the bool and the zeroed fields sit side by side, and nothing stops a careless surface reading the zero. That was a plan defect, not an implementation one. The refactor parcel converts it to `Analysis<LineCounts>` before 4b adds a second analysis that would copy the pattern.

## Rust idiom

The [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) are the baseline. The entries that bite most often here:

- **C-COMMON-TRAITS** — public data types derive `Debug`, `Clone`, `PartialEq`, and `Serialize` where they cross the JSON contract. `Eq` is kept derivable by keeping report fields integer and boolean; a float in the model breaks it, and a float in a *count* is a bug anyway.
- **C-NEWTYPE** — wrap a primitive when the wrapping prevents a real mistake. A `struct Bytes(u64)` earns its place the moment a function takes both a byte count and a line count; a newtype that only adds ceremony does not.
- **C-STRUCT-PRIVATE / `#[non_exhaustive]`** — `ScanReport` and `ScanConfig` gain fields every phase. Once [024](specs/024-view-layer.md) makes the UI a separate crate, they must be `#[non_exhaustive]` so a field addition is not a breaking change downstream. Within the defining crate literal construction still works, so our own tests are unaffected.
- **C-QUESTION-MARK** — propagate with `?`. Analyses degrade to `NotEvaluated`; they do not panic.
- **C-EXAMPLE** — public API items carry a runnable doc example. Doc tests are free integration tests and they are the ones that rot most visibly.

Beyond the guidelines, specific to this codebase:

- **Full words over abbreviations.** The existing code says `extension`, `character`, `directory`, not `ext`, `ch`, `dir`. Match it.
- **Doc comments explain *why*.** `sanitize_for_terminal` is the model: it says what a hostile repository could do with an escape sequence, not that it replaces characters. Anything security-relevant gets that treatment or it is incomplete.
- **Determinism is not a preference.** Sort explicitly. Never rely on directory order or hash iteration order. Two scans of one tree must be equal, and a test asserts it.
- **Bounded resource use on untrusted input.** Stream through fixed buffers. `read_to_end` on a file from a repository we did not write is an unbounded allocation and violates I9.

## Error handling

Today the library returns `io::Result`. That stops being honest the moment Git, manifests, and forge responses arrive, because "malformed `Cargo.toml`" is not an `io::Error`.

**Policy:** a hand-written crate error enum implementing `Display` and `std::error::Error`. No `thiserror`, no `anyhow`.

- `anyhow` in a library is the wrong tool regardless — it erases the type a caller needs to match on.
- `thiserror` is a fine crate and the derive is genuinely nicer. It is declined here because the error set is small, the crate's dependency budget is deliberately tight, and writing the impl by hand is squarely within this project's stated purpose of learning Rust.

Revisit if the variant count passes roughly twenty.

**Panics:** `unwrap` and `expect` are for tests and for invariants the type system cannot express, where the `expect` message states the invariant. `sanitize_for_terminal`'s callers are the pattern. In analysis paths, a failure is a `NotEvaluated` or a `ScanWarning` — never a panic, because a panic on a hostile repository is invariant I9 broken.

## Lints

Add to `src/lib.rs`, as its own parcel alongside the module refactor:

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::unwrap_used, clippy::expect_used)]
#![warn(clippy::todo, clippy::unimplemented)]
```

`#![forbid(unsafe_code)]` is the significant one. Repo Radar's central promise is that it is safe to point at an untrusted clone. Forbidding `unsafe` at the compiler level turns part of that promise from a claim into a property, and `forbid` rather than `deny` means it cannot be locally overridden.

`unwrap_used` and `expect_used` are `warn`, not `deny`, with `#[allow]` at test-module scope. A blanket denial would just teach everyone to write `#[allow]` everywhere, which is worse than the problem.

Also missing and worth adding in the same parcel: `rust-version` in `Cargo.toml`, so the MSRV is a declared fact rather than whatever the author happened to have installed.

## Testing

Four layers, each with a distinct job:

| Layer | Location | Job |
| --- | --- | --- |
| Unit | `#[cfg(test)]` in-module | Algorithms and edge cases, with private access |
| Integration | `tests/` | The binary's observable contract, through `Command` |
| Invariant | `tests/safety_invariants.rs` | Spec 000, via `assert_target_unchanged` |
| Doc | `///` examples | Public API stays usable and the examples stay true |
| Bench | `benches/` | Performance-sensitive changes, with a recorded baseline |

Rules:

- **Every command, on every path including failure, runs inside `assert_target_unchanged`.** Not negotiable, and it is the first thing a review checks.
- **A test asserts behavior, not implementation.** Asserting on a private helper's return couples the test to a refactor it should survive.
- **Never reach green by weakening a test.** Deleting an assertion, adding `#[ignore]`, or loosening a bound to make a failure go away is a defect being hidden. Stop and report instead — this is already a hard-stop condition for `seam-blaster`.
- **A test that cannot fail proves nothing.** The immutability harness tests itself for exactly this reason; new harnesses get the same treatment.

## Dependencies

The crate runs on `serde` and `serde_json`. [024](specs/024-view-layer.md) adds Dioxus, on a separate compile target, to a separate crate.

A new dependency requires the spec or build sheet that authorizes it to say so explicitly, and to say what was considered instead. This is not asceticism — it is that every dependency is a supply-chain surface on a tool whose entire pitch is being safe to run on code you have not read.

Pin exact versions for pre-1.0 crates. A bump is its own reviewed parcel with the green bar re-run.

## Dogfooding

[025 practice assessment](specs/025-practice-assessment.md) turns most of this document into a rule table Repo Radar evaluates against any repository — and this repository is its first fixture.

That is the point of writing it down this way. A guidelines document decays into fiction unless something checks it; and a tool that assesses engineering practice which cannot survive its own assessment has disqualified itself. When 025 lands, `repo-radar practices .` runs in this repository's CI, and the honest result is published in the README whether or not it is flattering.
