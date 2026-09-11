# Build sheet: The `Analysis` seam, crate lints, and MSRV — parcel 5b

Date: 2026-09-05
Spec: [ENGINEERING.md](../ENGINEERING.md), [002 structured output](../specs/002-structured-output.md), roadmap phase 5
Branch: `refactor/analysis-seam-and-lints`

## Goal

Close phase 5. Three things, in this order:

1. Replace `LineCounts { evaluated: bool, … }` with `Analysis<LineCounts>`, so invariant I10 is a
   property the compiler enforces rather than a convention every surface remembers.
2. Turn the four **Committed** rules in ENGINEERING.md's status table into **Enforced** ones by
   adding the crate lints.
3. Declare the MSRV in `Cargo.toml` and add the CI job that actually checks it.

Unlike 5a, this parcel is **not** behavior-neutral: the JSON document gains a `reason` field.
That change is additive within schema version 1 and is already specified — see
[002 structured output](../specs/002-structured-output.md), "Reporting an analysis that did not
run". Nothing else about the output changes.

**`tests/structured_output.rs`, `tests/cli.rs`, `tests/scan_engine.rs`, and
`tests/safety_invariants.rs` must pass completely unmodified.** They are the proof that the wire
contract held. If any of them needs an edit to go green, the implementation is wrong — stop and
report. Unit tests inside `src/` do change, and the exact edits are listed below.

## Why

**The seam.** ENGINEERING.md records this as known debt from parcel 4a: the `evaluated` bool and
the zeroed counts sit side by side in one struct, and nothing stops a surface reading the zero as
a measurement. Every analysis from spec 003 onward has the same shape — it ran, or it did not,
and it can be switched off. Parcel 4b adds Git basics and 4c adds Cargo parsing. Whichever
convention is in the tree when 4b lands is the one 4b copies, so the type has to exist first.

Encoding it as a type also buys something the bool cannot: **four distinguishable reasons.** "You
passed `--no-lines`", "there is no Git worktree here", "we cannot read Cursor logs yet", and "the
manifest is malformed" are four different facts a user needs told apart, and a bool collapses
them all into `false`.

**The lints.** `#![forbid(unsafe_code)]` is the significant one. Repo Radar's central promise is
that it is safe to point at an untrusted clone. `forbid` rather than `deny` means it cannot be
locally overridden, which turns part of that promise from a claim into a property.

**The MSRV.** ENGINEERING.md's own honesty rule is that the project does not pretend a rule is
holding when it is not. A `rust-version` nobody checks is exactly that failure, so the CI job
ships in the same parcel as the declaration.

## Seam contracts

### `Analysis<T>` and `NotEvaluated` — new, in `src/lib.rs`

Place these immediately above `LineCounts`, since `LineCounts` is their first user.

```rust
/// The result of an analysis that may not have been able to run.
///
/// Invariant I10 requires that an analysis which did not run reports
/// `not evaluated` rather than a plausible default. Making that a type rather
/// than a convention means a caller cannot render a zero where it meant
/// "unknown" — there is no zero to reach for, because there is no `T`.
///
/// ```
/// use repo_radar::{Analysis, NotEvaluated};
///
/// let ran: Analysis<u64> = Analysis::Ran(42);
/// assert_eq!(ran.ran(), Some(&42));
///
/// let skipped: Analysis<u64> = Analysis::NotEvaluated(NotEvaluated::Disabled);
/// assert_eq!(skipped.ran(), None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Analysis<T> {
    /// The analysis ran and produced this result.
    Ran(T),
    /// The analysis did not run, for the stated reason.
    NotEvaluated(NotEvaluated),
}

/// Why an analysis did not run.
///
/// The four variants are the four things a user needs told apart. Collapsing
/// them into a single "unavailable" would put "you switched this off" and "we
/// tried to read this and failed" in the same bucket.
#[derive(Debug, Clone, PartialEq, Eq)]
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

An analysis that was never populated has not run, so:

```rust
impl<T> Default for Analysis<T> {
    fn default() -> Self {
        Self::NotEvaluated(NotEvaluated::Disabled)
    }
}
```

Note the absent `T: Default` bound — `ScanReport` derives `Default` and must keep deriving it.

### Accessors

Keep the surface minimal. These two are all the renderers need:

```rust
impl<T> Analysis<T> {
    /// The result, or `None` if the analysis did not run.
    pub fn ran(&self) -> Option<&T>;
}

impl NotEvaluated {
    /// The stable wire token for this reason: `disabled`,
    /// `input_unavailable`, `unsupported`, or `failed`.
    pub fn reason(&self) -> &'static str;

    /// The specific input this reason names, when it names one.
    /// `Disabled` carries none.
    pub fn detail(&self) -> Option<&str>;
}
```

Do not add `is_evaluated`, `unwrap_or_default`, `map`, or a `Display` impl. Nothing in this
parcel needs them, and an accessor that lets a caller reach a zero without acknowledging the
`NotEvaluated` case reopens the hole the type closes.

### `LineCounts` after the change

The `evaluated` field is **deleted**. Everything else is unchanged, including the derives.

```rust
/// Line counting results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LineCounts {
    /// Total lines across all text files.
    pub lines: u64,
    /// Files read as text.
    pub text_files: usize,
    /// Files skipped as binary by the NUL-byte heuristic.
    pub binary_files: usize,
    /// Files that stat succeeded on but could not be opened.
    pub unreadable_files: usize,
}
```

`ScanReport.lines` becomes `pub lines: Analysis<LineCounts>`.

### The wire format

Ran:

```json
"lines": { "evaluated": true, "lines": 1234, "text_files": 10, "binary_files": 1, "unreadable_files": 0 }
```

Not evaluated:

```json
"lines": { "evaluated": false, "reason": "disabled", "lines": 0, "text_files": 0, "binary_files": 0, "unreadable_files": 0 }
```

Three properties this must hold, all of them load-bearing:

- **`evaluated` keeps its name, position, and meaning.** Existing consumers keep working.
- **`T`'s own fields are always present**, zero-valued when the analysis did not run. This is
  what keeps `tests/structured_output.rs`'s `assert_eq!(report["lines"]["lines"], 0)` passing
  without an edit. Do not "simplify" by omitting them — that is a schema break and it fails the
  test you are forbidden to weaken.
- **`reason` and `detail` appear only when `evaluated` is `false`.** `detail` additionally
  appears only when the variant carries one, so `Disabled` never emits it.

### The `Serialize` impl

This is the one genuinely fiddly piece. Write it exactly:

```rust
impl<T: Serialize + Default> Serialize for Analysis<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Wire<'a, T: Serialize> {
            evaluated: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            reason: Option<&'static str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            detail: Option<&'a str>,
            #[serde(flatten)]
            value: &'a T,
        }

        // A `NotEvaluated` analysis has no `T`, but the version 1 contract
        // requires the object keep its full field shape so a consumer's field
        // access never fails. The zero value stands in, and `evaluated: false`
        // is what marks those zeros as something other than measurements.
        let fallback;
        let wire = match self {
            Self::Ran(value) => Wire {
                evaluated: true,
                reason: None,
                detail: None,
                value,
            },
            Self::NotEvaluated(not_evaluated) => {
                fallback = T::default();
                Wire {
                    evaluated: false,
                    reason: Some(not_evaluated.reason()),
                    detail: not_evaluated.detail(),
                    value: &fallback,
                }
            }
        };
        wire.serialize(serializer)
    }
}
```

`fallback` is declared before `wire` so it outlives the borrow, and is initialized only on the
branch that reads it — that is legal and deliberate, not an oversight to tidy up.

The `T: Default` bound is on the `Serialize` impl only, not on the type or its `Default` impl.
`#[serde(flatten)]` requires `T` to serialize as a map; every analysis result is a struct, so
that holds, but say so in a doc comment on the impl.

## Files

| File | Change |
| --- | --- |
| `src/lib.rs` | Add the four crate lints. Add crate-level `//!` docs. Add `Analysis`, `NotEvaluated`, their impls, and the `Serialize` impl. Delete `LineCounts::evaluated`. Change `ScanReport.lines` to `Analysis<LineCounts>`. Thread a `LineCounts` accumulator through `scan_directory`. Document the 30 items listed under "Lint rollout". Update the four unit tests listed under "Tests". |
| `src/render/text.rs` | Replace the two `report.lines.evaluated` checks with one `ran()` binding. Document `write_summary`. |
| `src/render/json.rs` | No logic change — `LineCounts` becomes `Analysis<LineCounts>` in the `JsonReport` field type and serializes itself. Document `write_json`. Add the scoped `expect_used` allow. |
| `src/render/html/mod.rs` | Replace the `report.lines.evaluated` check with `ran()`. Document `write_html`. |
| `src/render/html/markup.rs` | Document `as_str` and `is_empty`. No logic change. |
| `src/render/mod.rs` | Add `#![allow(…)]` to its test module. No logic change. |
| `Cargo.toml` | Add `rust-version = "1.85"`. |
| `.github/workflows/ci.yml` | Add the `msrv` job. |
| `docs/ENGINEERING.md` | Flip four status-table rows to **Enforced**. |
| `docs/ROADMAP.md` | Phase 5 status → `Complete`. |
| `README.md` | Document the `lines` object and the `reason` field. |

No change to `tests/`, `benches/`, or the dependency list. No new dependency.

### `src/lib.rs` — the accumulator

`scan_directory` currently writes into `report.lines.*` at four sites (the `count_lines` match).
`report.lines` is no longer a struct with those fields, so the accumulator becomes a parameter:

```rust
fn scan_directory(
    root: &Path,
    directory: &Path,
    config: &ScanConfig,
    report: &mut ScanReport,
    line_counts: &mut LineCounts,
    directories: &mut BTreeMap<PathBuf, DirectoryEntry>,
    languages: &mut BTreeMap<String, LanguageStat>,
)
```

The four writes become `line_counts.lines += lines`, `line_counts.text_files += 1`,
`line_counts.binary_files += 1`, `line_counts.unreadable_files += 1`. The recursive call passes
`line_counts` through. Nothing else in the traversal changes.

In `scan`, delete the `report.lines.evaluated = config.count_lines;` line and its two-line
comment — the bug class it was guarding against no longer exists — then wrap once, after
traversal:

```rust
report.lines = if config.count_lines {
    Analysis::Ran(line_counts)
} else {
    Analysis::NotEvaluated(NotEvaluated::Disabled)
};
```

### `src/render/text.rs`

`write_summary` reads `evaluated` twice: once for the `Lines:` row, once inside the language loop
to decide whether to print the lines column. Bind once at the top and reuse:

```rust
let line_counts = report.lines.ran();
```

then `match`/`if let` on it at both sites. Output must stay byte-identical for both states.

### `src/render/json.rs` — the allow

`write_json` calls `serde_json::to_string(&output).expect("JSON report should serialize")`, which
`clippy::expect_used` now warns on. It is the documented exception in ENGINEERING.md's panic
policy — an invariant the type system cannot express — so keep the `expect` and scope the allow
to that statement, with the invariant stated:

```rust
// Every field is an owned, string-keyed, float-free value, so the only way
// serde_json fails here is a bug in a `Serialize` impl in this crate. That is
// the "invariant the type system cannot express" case in ENGINEERING.md's
// panic policy, not an error a caller could handle.
#[allow(clippy::expect_used)]
let serialized = serde_json::to_string(&output).expect("JSON report should serialize");
```

This is the **only** `allow` outside test scope in the parcel. If you find yourself wanting a
second one, stop and report instead.

## Lint rollout

Add to the top of `src/lib.rs`, above the `use` statements:

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::unwrap_used, clippy::expect_used)]
#![warn(clippy::todo, clippy::unimplemented)]
```

CI runs `clippy --all-targets --all-features -- -D warnings`, which promotes every one of those
warnings to an error. The measured blast radius is **35 warnings on the lib target** and a
further **57 on the lib test target**. Both must reach zero.

### Test modules

`unwrap` and `expect` are correct in tests. Add this as the first line inside the body of every
`#[cfg(test)] mod tests` in `src/`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used)]
```

Six modules need it: `src/lib.rs`, `src/render/mod.rs`, `src/render/text.rs`,
`src/render/json.rs`, `src/render/html/mod.rs`, `src/render/html/markup.rs`.

Files under `tests/` are separate crates and the lints do not reach them. Do not touch them.

### The 34 documentation sites

Measured against the current tree. Line numbers shift as you edit; the identity of each item does
not. `Analysis`, `NotEvaluated`, and their members are new and carry the docs given above, so
they are not in this list.

| File | Item |
| --- | --- |
| `src/lib.rs` | the crate itself — a `//!` header |
| `src/lib.rs` | `ScanConfig`, and its field `ignored_directories` |
| `src/lib.rs` | `FileEntry`, and its fields `path`, `bytes` |
| `src/lib.rs` | `LanguageStat`, and its fields `language`, `files`, `bytes` |
| `src/lib.rs` | `DirectoryEntry` |
| `src/lib.rs` | `LineCounts` fields `lines`, `text_files`, `binary_files`, `unreadable_files` |
| `src/lib.rs` | `ScanWarning`, and its fields `path`, `message` |
| `src/lib.rs` | `ScanReport`, and its eight fields |
| `src/lib.rs` | `scan` |
| `src/render/text.rs` | `write_summary` |
| `src/render/json.rs` | `write_json` |
| `src/render/html/mod.rs` | `write_html` |
| `src/render/html/markup.rs` | `as_str`, `is_empty` |

Some of these already have a non-doc `//` comment or a partial doc on a sibling field — for
example `LineCounts::lines` has none while the struct has one. Keep existing text, add what is
missing.

**Documentation says why, not what.** `/// The path.` on a field named `path` is noise and does
not count as done. `ScanReport.largest_directories` should say the root is excluded and the list
is sorted by aggregate bytes descending; `ScanConfig.ignored_directories` should say it is matched
against directory names, not paths, and name the defaults. `sanitize_for_terminal` is the model
to match. The crate `//!` header states what the library is, that it is read-only, and that
`docs/specs/000-safety-invariants.md` outranks everything.

Add a runnable doc example to `scan` and to `Analysis`. Doc tests run under `cargo test`, so a
broken example is a red bar. The other items get prose only — do not pad 30 items with examples.

## MSRV

`Cargo.toml` gains, in `[package]`:

```toml
rust-version = "1.85"
```

1.85 is the edition 2024 floor and nothing in the crate uses a newer API. **Do not raise it to
match the installed toolchain** (1.97.1) — that would declare an MSRV far stricter than the truth.

`.github/workflows/ci.yml` gains a second job so the number is checked rather than asserted:

```yaml
  msrv:
    name: Minimum supported Rust version
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.85.0
      - run: cargo check --all-targets --all-features
```

If a 1.85 toolchain can be installed locally (`rustup toolchain install 1.85.0`), run
`cargo +1.85.0 check --all-targets --all-features` and report the result in the receipt. If the
install is unavailable or blocked, say so in the receipt rather than claiming it passed — CI will
be the first real check, and an honest "unverified locally" is the correct receipt entry.

### Correction, 2026-09-10 — the floor is 1.88, not 1.85

**This section's premise was wrong and the shipped parcel does not follow it.** The claim that
"nothing in the crate uses a newer API" is false: `count_lines` in `src/lib.rs` uses a let-chain
(`if let Some(last) = last_byte && last != b'\n'`), and let-chains did not stabilize until
**1.88.0**. A 1.85-pinned `msrv` job would have failed on its first CI run with:

```
error[E0658]: `let` expressions in this position are unstable
```

Bisected against the tree at implementation time: **1.87.0 fails, 1.88.0 compiles clean**, both
verified with a full `cargo check --all-targets --all-features`.

The parcel ships `rust-version = "1.88"` and a `dtolnay/rust-toolchain@1.88.0` CI job. The
alternative — rewriting the let-chain to reach 1.85 — was rejected: it is out-of-scope code for
this parcel, nothing downstream needs 1.85, and let-chains are an edition-2024 idiom worth keeping
in a project whose purpose is learning Rust. Declaring a number CI would immediately disprove is
the exact failure this parcel exists to end.

## Tests

### Existing unit tests that change

All four are mechanical adaptations to a type change, not weakenings. Every assertion survives.

1. `binary_file_is_counted_but_not_line_counted` (`src/lib.rs`, the `binary_files` /
   `text_files` / `lines` assertions) — read through
   `report.lines.ran().expect("line counting was enabled")`.
2. `directory_totals_sum_descendant_files` (`src/lib.rs`) — delete the
   `report.lines.evaluated = config.count_lines;` line and pass a
   `&mut LineCounts::default()` as the new fifth argument to `scan_directory`.
3. `disabling_line_counting_reports_not_evaluated` (`src/lib.rs`) — this one gets **stronger**.
   Replace the four field assertions with:
   ```rust
   assert_eq!(
       report.lines,
       Analysis::NotEvaluated(NotEvaluated::Disabled),
       "a disabled analysis must name its reason, not report a zero"
   );
   ```
4. `unreadable_file_warns_and_counts_without_aborting` (`src/lib.rs`) — read `unreadable_files`
   through `ran().expect(…)`. The warning assertion is unchanged.

### New unit tests

Covering spec 002 acceptance criteria 8 and 9.

5. `analysis_that_ran_serializes_its_value_and_no_reason` — serialize
   `Analysis::Ran(LineCounts { lines: 7, text_files: 1, ..Default::default() })`, parse it, assert
   `evaluated == true`, `lines == 7`, and that `reason` and `detail` are both absent. **(AC 9)**
6. `analysis_that_did_not_run_reports_reason_and_zeroed_fields` — serialize
   `Analysis::<LineCounts>::NotEvaluated(NotEvaluated::Disabled)`, assert
   `evaluated == false`, `reason == "disabled"`, `lines == 0`, and `detail` absent. **(AC 8)**
7. `not_evaluated_detail_names_the_input` — serialize
   `NotEvaluated::Failed("Cargo.toml is not valid TOML".to_owned())` inside an `Analysis`, assert
   `reason == "failed"` and `detail == "Cargo.toml is not valid TOML"`. This variant has no
   producer yet; the test is what keeps the serializer path honest until 4c adds one.
8. `every_not_evaluated_reason_has_a_distinct_token` — assert the four `reason()` tokens are
   `disabled`, `input_unavailable`, `unsupported`, `failed`, and that they are all different.
9. `default_analysis_has_not_run` — assert `Analysis::<LineCounts>::default().ran().is_none()`.
10. `text_summary_says_not_evaluated_when_line_counting_is_off` (`src/render/text.rs`) — render a
    report whose `lines` is `NotEvaluated`, assert the output contains `Lines:      not evaluated`
    and that no language row carries a lines column.

### Tests that must pass untouched

Every file in `tests/`. `no_lines_flag_reports_not_evaluated_in_json` is the specific one that
proves the wire contract held: it asserts `report["lines"]["evaluated"] == false` **and**
`report["lines"]["lines"] == 0`. If that test fails, the `Default` fallback in the `Serialize`
impl is missing or wrong.

## Gotchas

1. **`#[serde(flatten)]` switches serde from `serialize_struct` to `serialize_map`.** That is
   fine for `serde_json` and is why field order in the emitted object follows the `Wire` struct's
   declaration order. Do not reorder `Wire`'s fields.
2. **`skip_serializing_if` on a flattened struct's siblings works, but only because they are
   `Option`.** Do not change `reason` to a plain `&str` with an empty-string sentinel.
3. **`scan_directory` reaches exactly 7 parameters.** Clippy's `too_many_arguments` threshold is
   7 and fires above it, so this compiles clean — but do not add an eighth. If a later parcel
   needs one, that parcel bundles the state into a struct.
4. **`#![forbid(unsafe_code)]` cannot be overridden anywhere in the crate**, including in a test.
   Nothing in the tree uses `unsafe` today, so this should be free. If it errors, something
   unexpected is in the tree — stop and report rather than downgrading it to `deny`.
5. **The crate-level `#![…]` attributes must precede every `use`**, including the existing
   `use std::collections::BTreeMap;` at line 1. An inner attribute after an item is a compile
   error.
6. **`missing_docs` only lints the lib target.** `src/main.rs` is a separate bin target and the
   attributes in `lib.rs` do not reach it. Do not add the lints to `main.rs`; the parcel's scope
   is the library.
7. **`main.rs:69` has an `expect` that stays.** It is in the bin target, out of the lints' reach,
   and 5a's sheet already justified it. Leave it alone.
8. **The JSON `Ran` case must not emit `reason: null`.** `skip_serializing_if` handles it; verify
   in the test rather than assuming.
9. **Do not delete `LineCounts`.** It stays as the `T`. Only its `evaluated` field goes.
10. **Text output for the evaluated case must stay byte-identical.** The `Lines:` row alignment
    and the language-row column widths are load-bearing on `format!` width specifiers; refactoring
    the two branches into one is how those get silently changed.
11. **`Analysis` must be re-exported.** It is used in `ScanReport`'s public field type, so it and
    `NotEvaluated` are public API. The doc example uses `repo_radar::Analysis` — confirm that path
    resolves.

## Green bar

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Plus, and report each in the receipt:

```bash
# Wire contract: the evaluated case must be unchanged from master apart from
# nothing at all, and the disabled case must gain only `reason`.
cargo run -- . --format json | jq -S '.lines'
cargo run -- . --format json --no-lines | jq -S '.lines'

# Text output, both states.
cargo run -- . --format text | head -8
cargo run -- . --format text --no-lines | head -8

# MSRV, if the toolchain can be installed.
cargo +1.85.0 check --all-targets --all-features
```

## Out of scope

- `scan/`, `analysis/`, and `sanitize.rs` module extraction — a later parcel.
- The hand-written crate error enum from ENGINEERING.md's error-handling policy. `io::Result`
  stays for now; that change lands with 4b, which is what makes it necessary.
- `#[non_exhaustive]` on `ScanReport` and `ScanConfig` — that belongs with phase 12, when the UI
  becomes a separate crate and the attribute starts doing something.
- Git basics (4b), Cargo parsing (4c).
- Any change to traversal, line counting, binary detection, language mapping, or HTML styling.

## Definition of done

- `ScanReport.lines` is `Analysis<LineCounts>`, and `LineCounts` has no `evaluated` field.
- Every file in `tests/` passes **unmodified**.
- JSON gains `reason` on the not-evaluated path and is otherwise byte-identical for both states.
- Text and HTML output byte-identical for both states.
- Six new unit tests and one new renderer test added and passing; the four adapted tests still
  assert everything they asserted before.
- The four crate lints are in `src/lib.rs` and
  `cargo clippy --all-targets --all-features -- -D warnings` is clean, with exactly one `allow`
  outside test scope.
- `rust-version = "1.85"` in `Cargo.toml`, and the `msrv` CI job exists.
- ENGINEERING.md's status table shows `No unsafe`, `MSRV declared`, `Public API documented`, and
  `No unwrap/expect outside tests` as **Enforced**, each naming the green bar and CI as its check.
- ROADMAP phase 5 reads `Complete`.
- README documents the `lines` object and `reason`.
- Green bar clean. No commit, no push.
