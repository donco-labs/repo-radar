# Build sheet: Cargo dependency view — parcel 4c

Date: 2026-09-13
Spec: [003 repository intelligence](../specs/003-repository-intelligence.md) (acceptance criteria 5, 6, 7 and the 4c row), [000 safety invariants](../specs/000-safety-invariants.md) (I3, I4, I6, I9, I10), [002 structured output](../specs/002-structured-output.md)
Branch: `feat/cargo-dependencies`

## Goal

Close phase 4. Read `Cargo.toml` and `Cargo.lock` at the scanned root and report:

1. **Direct dependencies** — name, version requirement, kind (normal, dev, build), and where they come from (registry, git, path, workspace inheritance).
2. **The locked package set** — how many packages the lockfile resolves to, and how many of those are local.

Together these satisfy AC 5: direct dependencies are enumerated by name, the lockfile gives the full resolved set, and the two are separately reported so a consumer can tell them apart.

Both are `Analysis<T>` and degrade independently. A repository with no `Cargo.toml`, a malformed one, or a manifest but no lockfile each produce a named reason rather than a zero.

## Why

This is the last parcel of phase 4, and the first analysis that parses a structured file format the repository author controls. Everything so far read bytes (lines), names (languages), or a subprocess's own stable output (git). A manifest is a *document format* with a grammar, which means a new failure mode — syntactically valid but semantically surprising input — and a new leak channel: parser error messages that quote the input back.

That second one turns out to be the interesting part of this parcel. See Safety.

## Decision: add the `toml` crate

**Authorized here, explicitly, per ENGINEERING.md's dependency rule**, which requires the build sheet to say so and to record what was considered instead.

```toml
toml = "1"
```

Cost, **measured against this tree** on 2026-09-13:

| Measure | Before | After |
| --- | --- | --- |
| **Crates actually compiled** | 12 | **18** |
| Lockfile entries | 12 | 21 |

The six compiled additions are `toml`, `serde_spanned`, `toml_datetime`, `toml_parser`, `toml_writer`, and `winnow`. That is the supply-chain surface: code that is linked into the binary.

The other three lockfile entries — `indexmap`, `hashbrown`, `equivalent` — are **never compiled**. `toml` declares `indexmap` as an optional dependency behind its `preserve_order` feature, which this crate does not enable, and Cargo records optional dependencies in `Cargo.lock` regardless. Verified: `cargo tree` and `cargo tree --all-features` reach none of the three.

> **Correction, twice over.** This sheet first said "5 new transitive crates, 12 → 17", measured with `cargo add toml` in a throwaway project — an undercount, because it omitted `toml` itself. It was then corrected to "9 new crates, 12 → 21, a 75% increase", read straight off this repository's lockfile diff — an overcount, because three of those entries are optional dependencies that never compile.
>
> Two lessons, both earned the hard way. **Measure in the tree that will carry the dependency, not in a scratch project.** And **a lockfile entry is not the same as a compiled crate** — `Cargo.lock` records the resolution graph including optional dependencies, so counting its lines overstates what actually gets built. `cargo tree` is the honest instrument; the lockfile is not.

Considered and declined:

- **Hand-rolling a TOML subset.** The attraction is zero new supply-chain surface on a tool pitched as safe to point at unread code, and TOML-by-hand is squarely within this project's learning purpose. Declined on measured evidence: of 290 real manifests sampled from the local registry cache, **225 (78%) use the `[dependencies.NAME]` table form** and **74 (26%) use `[target.'cfg(...)'.dependencies]`**. Supporting both, plus inline tables, quoted keys, and comments, is most of a TOML parser. A subset parser that reported `NotEvaluated::Failed` on a quarter of real repositories would be honest but weak, and one that guessed would be worse than weak.
- **`cargo metadata`.** Rejected outright and permanently: it resolves dependencies, can reach the network, and can run build scripts. That is invariants I3 and I6 broken in a single subprocess. Do not revisit this.
- **`cargo_toml` / `cargo-lock` crates.** Purpose-built and would work, but each carries `toml` underneath anyway, so they add a layer without removing the dependency being weighed.

Version pinned as `"1"`, matching `serde` and `serde_json`. ENGINEERING.md's exact-pin rule applies to pre-1.0 crates; `toml` is past 1.0.

## Safety

### The parser error messages leak repository content — verified

This is the finding that shapes the parcel. `toml`'s error types quote the offending input back, and **both** the obvious channel and the non-obvious one leak.

`Display` echoes the source line verbatim, escapes and all:

```
TOML parse error at line 3, column 9
  |
3 | evil = ^[[31mAPI_KEY_sk_live_abc123 unterminated
  |         ^
unexpected key or value, expected newline, `#`
```

That is untrusted repository content, containing a live ANSI escape, on a path to the terminal. Spec 000 criterion 6 already caught this exact defect once, in file names.

`Error::message()` looks like the safe alternative and **is not**:

```
message(): "invalid type: string \"sk_live_SECRET_VALUE\", expected u32"
```

Serde's type errors embed the offending value. Measured directly; do not assume a message is content-free because one example was.

**The rule, therefore:**

> Never put `toml::de::Error`'s `Display` **or** its `message()` into the report, a warning, or any output. Use `Error::span()` — a byte range — and convert it to a line number with our own code. The `NotEvaluated` detail is a tool-authored constant plus that line number.

`span()` returns `Option<Range<usize>>` (measured: `Some(30..30)` for the case above). Counting newlines in the input up to `span.start` gives a 1-based line. A detail of `"Cargo.toml is not valid TOML (line 3)"` is useful to the user and carries nothing from the file.

This is the same rule 4b applied to git's stderr, arrived at independently and for the same reason. It is becoming a project-wide invariant: **diagnostic text from a parser or subprocess is untrusted content, not a message.** Say so in the module docs.

### Dependency names and versions are untrusted

Everything parsed out of a manifest — package names, version requirements, `cfg()` target expressions — is attacker-controlled. It enters the report and reaches the terminal, so **every one of these fields must go through `sanitize_for_terminal` in the text and HTML renderers**, the way language names already do. A crate named `"\x1b[31mevil"` is a legal TOML string.

### Bounded reads (I9)

`fs::read_to_string` on a file from an untrusted repository is an unbounded allocation. Cap both files at **4 MiB**; over that, report `NotEvaluated::Failed` with `"Cargo.toml exceeds the size limit"` rather than reading it. For scale, the largest manifest in the sampled registry cache is far under this.

Non-UTF-8 content reports `Failed("Cargo.toml is not valid UTF-8")`, not a panic.

### Paths

Both files are `root.join("Cargo.toml")` and `root.join("Cargo.lock")` — fixed names joined to the scanned root, never a path taken from file content. No traversal is possible (I8). Nothing is executed (I3). No network (I6).

## Seam contracts

New module `src/analysis/cargo.rs`, alongside `git.rs`. `src/analysis/mod.rs` gains `pub mod cargo;`.

### Result types

```rust
/// What the manifest at the scanned root declares.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CargoManifest {
    /// The package name, absent for a virtual workspace manifest.
    pub package_name: Option<String>,
    /// The package version, absent for a virtual workspace manifest.
    pub package_version: Option<String>,
    /// True when the manifest declares a `[workspace]` table.
    pub workspace_root: bool,
    /// Direct dependencies, sorted by kind then name. Untrusted content:
    /// every string here came from the manifest and must be sanitized
    /// before it reaches a terminal.
    pub dependencies: Vec<CargoDependency>,
}

/// One declared dependency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CargoDependency {
    /// The dependency name as written in the manifest.
    pub name: String,
    /// The declared version requirement. Absent for a git, path, or
    /// workspace-inherited dependency, which state no version of their own.
    pub requirement: Option<String>,
    /// Which dependency table this came from.
    pub kind: DependencyKind,
    /// The `cfg(...)` expression, when the dependency is target-specific.
    pub target: Option<String>,
    /// Where the dependency resolves from.
    pub source: DependencySource,
}

/// Which dependency table a dependency was declared in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyKind { Normal, Dev, Build }

/// Where a dependency resolves from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencySource { Registry, Git, Path, Workspace }

/// What the lockfile resolved to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CargoLock {
    /// The lockfile format version, when it declares one.
    pub lock_version: Option<u32>,
    /// Total packages in the lockfile.
    pub packages: usize,
    /// Packages with no `source` — workspace members and path dependencies,
    /// which are part of this repository rather than fetched.
    pub local_packages: usize,
}
```

`Default` on `CargoManifest` and `CargoLock` is required by `Analysis<T>`'s `Serialize` bound. `CargoDependency` needs no `Default` — it only ever appears inside a `Vec`.

### `ScanReport` and `ScanConfig`

```rust
    /// What `Cargo.toml` declares, or why it was not read.
    pub cargo_manifest: Analysis<CargoManifest>,
    /// What `Cargo.lock` resolved to, or why it was not read.
    pub cargo_lock: Analysis<CargoLock>,
```

```rust
    /// Read Cargo manifests. Defaults to true.
    pub read_cargo: bool,
```

Two analyses, not one, for the same reason 4b split status from activity: a manifest with no committed lockfile is an ordinary repository, and one combined analysis would have to collapse "read the manifest fine, there is no lockfile" into a single reason and lose half of it.

### Entry point

```rust
/// Reads the Cargo manifest and lockfile at `root`.
///
/// Never returns an error: a missing, oversized, non-UTF-8, or malformed
/// file becomes a `NotEvaluated` reason, so a non-Cargo repository still
/// produces a successful report.
pub fn analyze(root: &Path, config: &ScanConfig) -> (Analysis<CargoManifest>, Analysis<CargoLock>);
```

## Reason mapping

Each row is reachable and gets a test.

| Condition | Result |
| --- | --- |
| `read_cargo` is false | `NotEvaluated(Disabled)` for both |
| `Cargo.toml` absent | manifest `InputUnavailable("no Cargo.toml at the repository root")` |
| `Cargo.lock` absent | lock `InputUnavailable("no Cargo.lock at the repository root")` |
| File over 4 MiB | `Failed("Cargo.toml exceeds the size limit")` |
| Not valid UTF-8 | `Failed("Cargo.toml is not valid UTF-8")` |
| Not valid TOML | `Failed("Cargo.toml is not valid TOML (line N)")` — line from `span()` only |
| Valid TOML, unexpected shape | `Failed("Cargo.toml is not a valid Cargo manifest (line N)")` |
| Otherwise | `Ran(..)` |

The manifest and the lockfile are read independently: either can be `Ran` while the other is not.

## Parsing

Deserialize with `serde`, which is already a dependency. **All five dependency-declaration shapes were verified to parse through one `#[serde(untagged)]` enum** — this is measured, not hoped:

```toml
simple    = "1"                                  # Version("1")
inline    = { version = "2", features = ["a"] }  # Detailed, version Some
inherited = { workspace = true }                 # Detailed, version None
gitdep    = { git = "https://..." }              # Detailed, version None
[dependencies.tableform]                         # Detailed, version Some
version = "3"
```

```rust
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawManifest {
    package: Option<RawPackage>,
    workspace: Option<toml::Value>,
    #[serde(default)] dependencies: BTreeMap<String, RawDependency>,
    #[serde(default)] dev_dependencies: BTreeMap<String, RawDependency>,
    #[serde(default)] build_dependencies: BTreeMap<String, RawDependency>,
    #[serde(default)] target: BTreeMap<String, RawTarget>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawDependency {
    Version(String),
    Detailed(RawDetailed),
}
```

Notes, each verified:

- `#[serde(rename_all = "kebab-case")]` maps `dev_dependencies` to `dev-dependencies` correctly.
- Unknown tables and fields are **ignored** — do not add `deny_unknown_fields`. Manifests carry `[lints]`, `[features]`, `[[bench]]`, and whatever Cargo adds next; rejecting them would fail on ordinary repositories.
- `target` keys are the raw `cfg(...)` expression, e.g. `cfg(unix)`. Carry it into `CargoDependency.target` as untrusted content.
- `DependencySource` is derived: `workspace: Some(true)` → `Workspace`; else `git` present → `Git`; else `path` present → `Path`; else `Registry`.

Lockfile:

```rust
#[derive(Deserialize)]
struct RawLock {
    version: Option<u32>,
    #[serde(default)] package: Vec<RawLockPackage>,
}
#[derive(Deserialize)]
struct RawLockPackage { name: String, version: String, source: Option<String> }
```

Verified against this repository's own lockfile: `version: Some(4)`, 12 packages, and `source: None` identifies local packages.

`dependencies` must be **sorted by `(kind, name)`** explicitly before being stored — ENGINEERING.md's determinism rule. `BTreeMap` gives name order within a table, but the three tables are concatenated and target-specific deps are appended, so the final sort is not free.

## Files

| File | Change |
| --- | --- |
| `Cargo.toml` | Add `toml = "1"`. |
| `src/analysis/cargo.rs` | New. Everything above, plus unit tests. |
| `src/analysis/mod.rs` | `pub mod cargo;`. |
| `src/lib.rs` | Re-export the five public types. Two `ScanReport` fields, one `ScanConfig` field. Call `analysis::cargo::analyze` in `scan`. |
| `src/render/text.rs` | A `Cargo:` block. **Sanitize every name, requirement, and target.** |
| `src/render/json.rs` | Two `JsonReport` fields. Additive within schema version 1. |
| `src/render/html/mod.rs` | A Cargo section, same sanitizing. |
| `src/main.rs` | `--no-cargo`. Help text. |
| `tests/common/mod.rs` | Fixture helpers — see Tests. |
| `tests/safety_invariants.rs` | The malformed-manifest and hostile-name tests. |
| `tests/structured_output.rs` | JSON assertions for ran and not-evaluated. |
| `docs/specs/003-repository-intelligence.md` | Mark 4c delivered; `Status: Implemented`; record the parser-error-leak finding under Clarifications. |
| `docs/ROADMAP.md` | **Phase 4 → `Complete`.** |
| `docs/ENGINEERING.md` | Note `toml` in the Dependencies section, with the rationale summarized. |
| `README.md` | Document the Cargo fields and `--no-cargo`. |

## Tests

### Unit, in `src/analysis/cargo.rs`

1. `parses_every_dependency_declaration_shape` — the five shapes above; assert name, requirement, and `DependencySource` for each.
2. `parses_dependency_kinds` — `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` map to the right `DependencyKind`.
3. `parses_target_specific_dependencies` — `[target.'cfg(unix)'.dependencies]` yields `target: Some("cfg(unix)")`.
4. `dependencies_are_sorted_by_kind_then_name` — deliberately unsorted input, assert deterministic order.
5. `virtual_workspace_manifest_has_no_package` — `[workspace]` with no `[package]` gives `package_name: None`, `workspace_root: true`, and is `Ran`, not `Failed`.
6. `unknown_tables_are_ignored` — a manifest with `[lints]`, `[features]`, and an invented table still parses.
7. `lockfile_counts_packages_and_local_packages` — a lockfile with a mix of sourced and sourceless packages.
8. `malformed_toml_reports_the_line_and_nothing_else` — **the important one.** Given the hostile manifest below, assert the detail contains `line 3` and does **not** contain `API_KEY`, `sk_live`, or a `\x1b` byte:
   ```
   [package]
   name = "ok"
   evil = \x1b[31mAPI_KEY_sk_live_abc123 unterminated
   ```
9. `type_error_does_not_leak_the_offending_value` — a manifest where a field has the wrong type and the value is `"sk_live_SECRET_VALUE"`; assert the detail does not contain it. This is the `message()` channel, which is the one that looks safe and is not.
10. `oversized_manifest_is_rejected_before_parsing` — assert the size check fires without the content being parsed.

### Integration

All inside `assert_target_unchanged`.

11. `cargo_analyses_report_dependencies_in_json` — this repository's own shape: a manifest with a simple and an inline dependency, plus a lockfile.
12. `no_cargo_flag_reports_not_evaluated` — AC 7, both analyses, `reason == "disabled"`, fields zeroed.
13. `non_cargo_directory_still_produces_a_report` — `Fixture::typical()` has a `Cargo.toml`; use a fixture without one. Exit 0, both `input_unavailable`.
14. `malformed_manifest_does_not_abort_the_scan` — **AC 6.** The scan exits 0, the manifest analysis is `failed`, and the rest of the report is complete.
15. `i4_hostile_dependency_name_does_not_reach_output_unsanitized` — a manifest declaring a dependency whose name contains an ANSI escape; assert no raw escape byte in text output. This is the sanitizing requirement, tested rather than asserted.

### Fixture helpers

Add `cargo_fixture_malformed()` and `cargo_fixture_hostile_dependency_name()` to `tests/common/mod.rs`, following the existing `git_fixture_*` convention.

## Out of scope

- **The hand-written crate error enum.** Predicted by the 5b sheet for 4b, then by the 4b sheet for 4c. **It is not forced here either, and this sheet stops deferring the question.** Every parse failure degrades to `NotEvaluated`, so nothing propagates out of `scan` and its `io::Result` signature is untouched. The enum should be decided on its own merits in its own parcel rather than predicted into the next one a third time — raise it with the user after phase 4 closes.
- Version staleness, semver comparison, licence checks, advisories — [017 dependency intelligence](../specs/017-dependency-intelligence.md), phase 22. **Do not add a `resolved_version` field cross-referencing the lockfile.** That is 017's model to design, and inventing a thin version of it here means 017 has to migrate it.
- The module graph and cycle detection — [010](../specs/010-dependency-graph.md).
- Non-Cargo ecosystems — `package.json`, `pyproject.toml`, `go.mod`. Spec 014 project profile, phase 6.
- Workspace member traversal. Only the root manifest and lockfile are read. A virtual workspace reports its members' absence honestly rather than walking into them.

## Gotchas

1. **Never `Display` or `message()` a `toml` error into the report.** `span()` plus a tool-authored constant. Both leak channels are verified; this is the parcel's central rule.
2. **Check the file size before reading, not after.** `fs::metadata` first. Reading 2 GiB to then reject it is the allocation I9 forbids.
3. **Do not add `deny_unknown_fields`.** Real manifests carry tables we do not model.
4. **`Fixture::typical()` already contains a `Cargo.toml`** (`[package]\nname = "fixture"\n`). Test 13 needs a fixture without one — build it explicitly rather than assuming.
5. **`toml = "1"` changes `Cargo.lock`**, which is committed. Expect a lockfile diff of 5 added packages and include it in the commit; do not `.gitignore` it.
6. **The MSRV job must still pass.** `toml` 1.x and `winnow` 1.x have their own MSRVs. If `cargo +1.88.0 check` fails after adding the dependency, **stop and report** — do not raise `rust-version` to make it pass. That is the failure mode 5b existed to end, and the fix might be pinning an older `toml` instead.
7. **Sanitize on render, not on parse.** The model holds what the manifest said; the renderer makes it safe for its medium. Sanitizing at parse time would corrupt the JSON contract, where escapes are already handled by JSON string encoding.
8. **`Eq` must stay derivable.** All fields are `String`, `usize`, `u32`, `Option`, and C-like enums. No floats.
9. **`DependencyKind` derives `Ord`** for the sort; `DependencySource` does not need it.
10. **Existing output must stay byte-identical** apart from the additive fields. `tests/cli.rs` passes unmodified.

## Green bar

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Plus, reported in the receipt:

```bash
# Both analyses against this repository, which is a Cargo project.
cargo run -- . --format json | jq -S '.cargo_manifest, .cargo_lock'
cargo run -- . --format json --no-cargo | jq -S '.cargo_manifest, .cargo_lock'

# A directory with no manifest must still exit 0.
mkdir -p /tmp/rr-nocargo && cargo run -- /tmp/rr-nocargo --format json | jq -S '.cargo_manifest'; echo "exit=$?"

cargo run -- . --format text | head -30

# MSRV — see gotcha 6. Stop and report if this fails.
cargo +1.88.0 check --all-targets --all-features

# The new dependency tree, for the receipt.
cargo tree --depth 1
```

## Definition of done

- `ScanReport` carries `cargo_manifest` and `cargo_lock`, both `Analysis<T>`.
- All five dependency shapes, three kinds, and target-specific dependencies parse correctly.
- **No `toml` error text reaches the report.** Tests 8 and 9 both pass, and each has been observed failing against an implementation that uses `Display`/`message()` — verify deliberately and report both results.
- Every dependency name, requirement, and target expression is sanitized in the text and HTML renderers, with test 15 proving it.
- A malformed manifest does not abort the scan (AC 6); a non-Cargo directory exits 0 (AC 7 for the disabled case).
- `cargo +1.88.0 check` still clean, with `rust-version` unchanged.
- `tests/cli.rs` unmodified.
- Spec 003 `Status: Implemented`, all six acceptance criteria met, 4c row delivered.
- **ROADMAP phase 4 → `Complete`.**
- ENGINEERING.md's Dependencies section records `toml` and why.
- Green bar clean. No commit, no push.
