# Build sheet: Project purpose — parcel 6a

Date: 2026-09-13
Spec: [014 project profile](../specs/014-project-profile.md) (acceptance criteria 1, 2, 5, 6, 8 and the 6a row), [000 safety invariants](../specs/000-safety-invariants.md) (I4, I8, I9, I10), [002 structured output](../specs/002-structured-output.md)
Branch: `feat/project-purpose`
PR: #11

## Goal

Open phase 6. Answer the first half of "what is this" — a **stated purpose**, with the file it came from and how much that file's word is worth.

One new analysis, `Analysis<ProjectPurpose>`, extracted from a versioned precedence table:

1. A manifest description field — `certain`, because a manifest is a declaration.
2. The README's first substantive paragraph — `inferred`, because prose is prose.
3. A non-placeholder `.git/description` — `inferred`.

Nothing is invented. A repository with none of the three reports `not evaluated` and names what it looked for, rather than an empty statement a surface could render as a fact.

## Why this parcel, and why it is first in phase 6

Spec 014 has two halves. This one is small and establishes a seam; the other (6c, the stack detector table) is large and consumes it. `Confidence` and the evidence-path requirement are criteria 1 and 2, which both halves are held to. Defining them once on a single finding — where there is exactly one thing to get right — is much cheaper than defining them across ten stack categories at the same time as the categories themselves.

The parcel is also the first to read a *prose* file. Everything so far read bytes, names, a subprocess's stable output, or a document with a grammar. A README has no grammar, which means the extraction rules have to be written down precisely or two implementations will disagree. They are written down below, exhaustively, for that reason.

## No new dependencies

`serde_json` reads `package.json` and `composer.json`. `toml` reads `Cargo.toml` and `pyproject.toml`. `go.mod` is line-oriented and hand-parsed in about ten lines. The README is plain text.

**`*.csproj` from the spec's manifest list is deliberately NOT implemented.** It needs an XML parser, and no XML parser is authorized here. Do not add one, do not substring-scan for `<Description>`, and do not improvise. Spec 014's Clarifications record the decision; the README accuracy note records it for users.

**If you find yourself wanting a crate for anything in this parcel, STOP and report.** That is a decision, not an implementation detail.

## Safety

### The statement is untrusted repository content (I4)

A manifest `description` and a README paragraph are attacker-controlled strings, exactly like a file name or a dependency name. They enter the report and reach the terminal.

- **Sanitize on render, never on extraction.** `sanitize_for_terminal` in the text renderer, `Html::escape` in the HTML renderer. The model holds what the file said. Sanitizing at extraction would corrupt the JSON contract, where escapes are already handled by JSON string encoding. This is the rule parcel 4c established; follow it exactly.
- Test 20 proves it end to end with an ANSI escape inside a `description` field.

### Parser diagnostics are untrusted content, not messages

The project-wide rule, arrived at independently for git's stderr (4b) and `toml::de::Error` (4c), and it applies again here to **both** `toml` and `serde_json`:

> Never put a parser's `Display`, `to_string()`, or `message()` into the report, a warning, or any output.

`serde_json::Error`'s `Display` quotes input the same way `toml`'s does. In this parcel the only thing a parse failure produces is a **tool-authored constant** naming the file. No line numbers are needed here — unlike 4c, a malformed manifest is not the analysis's result, it is a source that gets skipped — so nothing at all is taken from the error value.

### Bounded reads (I9)

Every file is size-checked with `fs::metadata` **before** it is read.

| File | Cap |
| --- | --- |
| Manifests (`Cargo.toml`, `package.json`, `pyproject.toml`, `composer.json`, `go.mod`) | 4 MiB, the existing `MAX_FILE_BYTES` |
| README candidates | 1 MiB |
| `.git/description` | 64 KiB |

Over the cap, the source is skipped (and warned about for a manifest) rather than read. Non-UTF-8 content is likewise a skip, never a panic.

### Paths (I8)

Every path is a **fixed name joined to the scanned root**. No path is built from file content. The one `read_dir` — the README candidate lookup, one level, no recursion — inspects `file_name()` only and never follows an entry that is a symlink or a directory.

`.git/description` is read as a file under the scanned root. It is a read; nothing is written, and the spec 000 harness's `.git`-byte-identical assertion must still pass.

### Degradation (I10)

`analyze` never returns an error. Every failure is either a skipped source or a `NotEvaluated` reason. A malformed `Cargo.toml` must not stop `package.json` from being tried, and must not stop the scan.

## Seam contracts

### Shared: `Confidence` and `read_bounded` move into `src/analysis/mod.rs`

Both are cross-analysis and both are needed by 6c. Put them in the seam module now rather than in `profile.rs` where 6c would have to reach sideways for them.

```rust
/// How much weight a finding's evidence carries.
///
/// Spec 014 criterion 2: `Certain` is reserved for a value a manifest or a
/// lockfile *declares*. Anything read out of prose, inferred from a naming
/// convention, or produced by a heuristic is `Inferred`, however obvious it
/// looks. The distinction is the difference between reporting evidence and
/// reporting a guess, which is the line spec 014 exists to hold.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Declared by a manifest or lockfile entry.
    Certain,
    /// Read from prose, a naming convention, or a heuristic.
    #[default]
    Inferred,
}

impl Confidence {
    /// The stable lowercase token, matching the JSON contract's spelling.
    pub fn label(self) -> &'static str { /* "certain" | "inferred" */ }
}
```

`Inferred` is the `Default` deliberately: the weaker claim is the safe one to fall into.

`read_bounded` is lifted verbatim out of `src/analysis/cargo.rs` and generalized by one parameter:

```rust
/// Reads `path` as UTF-8 text, refusing anything over `max_bytes` via
/// [`fs::metadata`] *before* the content is read (invariant I9). `label` is
/// the file's own name, used only to build a tool-authored detail string —
/// never anything read from the file.
pub(crate) fn read_bounded(
    path: &Path,
    label: &str,
    max_bytes: u64,
) -> Result<String, NotEvaluated>;
```

**The four detail strings it produces must stay byte-identical** — `"no {label} at the repository root"`, `"{label} exceeds the size limit"`, `"{label} could not be read"`, `"{label} is not valid UTF-8"`. `cargo.rs` deletes its private copy, keeps its `MAX_FILE_BYTES` constant (now `pub(crate)`), and calls the shared one. **Every existing cargo test must pass unmodified.** If one does not, the move changed behavior — stop and report.

### New module `src/analysis/profile.rs`

```rust
/// A stated purpose for the repository, with the evidence that produced it.
///
/// Every string here is untrusted repository content and must be sanitized
/// by the renderer for its medium, never at extraction time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProjectPurpose {
    /// The stated purpose, whitespace-normalized and capped.
    pub statement: String,
    /// The repository-relative file the statement came from. Criterion 1
    /// makes a finding with no evidence path a defect, so this is not an
    /// `Option`.
    pub evidence: PathBuf,
    /// Which kind of source produced it.
    pub source: PurposeSource,
    /// `Certain` for a manifest field, `Inferred` for prose.
    pub confidence: Confidence,
    /// True when the statement was cut at [`MAX_STATEMENT_CHARS`].
    pub truncated: bool,
}

/// Which kind of source a purpose statement came from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PurposeSource {
    /// The wire zero `Analysis<T>`'s contract requires for a result that did
    /// not run. Never produced by a successful extraction.
    #[default]
    Undetermined,
    /// A package manifest's description field.
    Manifest,
    /// The README's first substantive paragraph.
    Readme,
    /// `.git/description`.
    GitDescription,
}

impl PurposeSource {
    /// The stable snake_case token, matching the JSON contract's spelling.
    pub fn label(self) -> &'static str {
        /* "undetermined" | "manifest" | "readme" | "git_description" */
    }
}

/// Version of the purpose precedence table. Bump on any entry or order change.
pub const PURPOSE_TABLE_VERSION: u32 = 1;

/// The cap on a statement's length, in characters.
pub const MAX_STATEMENT_CHARS: usize = 500;
```

`PurposeSource::Undetermined` exists so that a `NotEvaluated` purpose serializes as `"source": "undetermined"` rather than a plausible-looking `"manifest"`. `Analysis<T>`'s wire contract fills absent results with `T::default()`, and a default that reads as a real measurement is the exact thing I10 forbids. It is never constructed by a successful extraction, and no renderer ever sees it, because renderers only reach `ProjectPurpose` through `Analysis::Ran`.

### Entry point

```rust
/// Extracts the repository's stated purpose, in the precedence order spec
/// 014 defines.
///
/// Never returns an error. A source that is absent, oversized, non-UTF-8, or
/// malformed is skipped — a malformed manifest additionally producing a
/// warning (criterion 6) — and the next source is tried. When no source
/// yields a statement, the result is `NotEvaluated`, not an empty one
/// (invariant I10).
pub fn analyze(
    root: &Path,
    config: &ScanConfig,
) -> (Analysis<ProjectPurpose>, Vec<ScanWarning>);
```

The `Vec<ScanWarning>` return is new for this tree — `git::analyze` and `cargo::analyze` return only analyses. It is required by criterion 6, which says a malformed manifest *warns*. `scan` appends them to `report.warnings` **after** traversal, so warning order stays deterministic.

### `ScanReport` and `ScanConfig`

```rust
    /// The repository's stated purpose, or why none was found.
    pub purpose: Analysis<ProjectPurpose>,
```

```rust
    /// Run the project profile analyses. Defaults to true.
    pub read_profile: bool,
```

Named `read_profile`, not `read_purpose`: parcel 6c adds the stack detector behind the same switch, and renaming a public config field later is a breaking change for no gain.

## The precedence table

A single versioned, ordered table. The first source that yields a non-empty normalized statement wins; later sources are not consulted.

| # | Source | Field | Evidence path | Confidence |
| --- | --- | --- | --- | --- |
| 1 | `Cargo.toml` | `[package].description` | `Cargo.toml` | `Certain` |
| 2 | `package.json` | `.description` | `package.json` | `Certain` |
| 3 | `pyproject.toml` | `[project].description`, else `[tool.poetry].description` | `pyproject.toml` | `Certain` |
| 4 | `composer.json` | `.description` | `composer.json` | `Certain` |
| 5 | `go.mod` | the `module` path | `go.mod` | `Certain` |
| 6 | README | first substantive paragraph | the README's own name | `Inferred` |
| 7 | `.git/description` | whole file, if not the placeholder | `.git/description` | `Inferred` |

Notes, each of which is a requirement:

- **A source that exists but states nothing is a miss, not a stop.** `Cargo.toml` with no `description` key falls through to `package.json`. This repository's own `tests/common/mod.rs::Fixture::typical()` is exactly that case.
- **A non-string value is a miss.** `description.workspace = true` in a Cargo manifest is a table, not a string; `as_str()` returns `None` and the source falls through. Do not chase the workspace inheritance — that is out of scope and would mean reading a second manifest.
- **A malformed manifest is a miss plus a warning** (criterion 6). Not a stop, not a `Failed` result.
- **`go.mod`'s module path is a name, not a sentence**, and it sits last among the manifests for that reason. It is still `Certain`: the manifest declares it.

### `go.mod` parsing

Line-oriented, no crate. Scan at most the first 200 lines for the first line whose trimmed form starts with `module` followed by ASCII whitespace; the statement is the remainder, trimmed, with a trailing `// comment` removed. A `module (` block form is valid Go syntax but vanishingly rare — treat a remainder of `(` as a miss rather than parsing the block.

### `.git/description`

`root.join(".git")` must be a directory (a worktree or submodule makes it a file — that is a miss). Read `root.join(".git/description")` bounded at 64 KiB. Skip it when the normalized statement starts with `Unnamed repository`, which is Git's own placeholder.

## README extraction (criterion 8)

This is the part with no grammar, so the rules are exhaustive and ordered. Implement them in this order.

### Choosing the file

`read_dir(root)`, one level, no recursion. Consider only entries whose `file_type()` is a plain file — not a symlink, not a directory. Lowercase each `file_name()` and keep those in the candidate table:

```text
readme.md, readme.markdown, readme.rst, readme.txt, readme
```

Pick the match earliest in that table; on a tie (two files differing only in case, possible on a case-sensitive filesystem) pick the lexicographically smaller actual file name. **Determinism is not a preference** — a tie broken by directory order is a defect.

The evidence path is the file's actual name as it appears on disk, not the lowercased form.

### Finding the paragraph

Work on the bounded UTF-8 content. First, **remove HTML comments**: every `<!--` through its matching `-->`, including across line breaks. An unterminated `<!--` removes the remainder of the file.

Then walk the remaining lines, skipping while the trimmed line is any of:

1. Empty.
2. A fenced code delimiter — starts with ` ``` ` or `~~~`. Toggle "inside a fence"; skip every line while inside, including the closing delimiter.
3. An ATX heading — starts with `#`.
4. A Setext underline — consists only of `=`, or only of `-`, and is at least two characters.
5. A horizontal rule — three or more of `-`, `*`, or `_` with only whitespace between.
6. An HTML block — starts with `<`.
7. A table row — starts with `|`.
8. **Badge-only** — see below.

The first line that is none of these begins the paragraph.

### Badge-only lines

Strip, repeatedly, every Markdown image (`![...](...)`), every Markdown inline link (`[...](...)`), and every Markdown reference link or image (`![...][...]`, `[...][...]`). If what remains is whitespace or only the characters `()[]!<>|-` , the line was badges and is skipped.

Implement the stripping as a small scanner over the characters, not with nested `find`/`replace` passes that can loop. A malformed construct (an unclosed `](`) must terminate the scan and leave the rest of the line as-is; **it must not hang**. A README is untrusted input and I9 applies to it.

### Building the statement

From the first substantive line, take consecutive lines until one is empty, is an ATX heading, is a fence delimiter, or is a table row. Join with a single space.

Then **normalize**: collapse every run of Unicode whitespace to one space, trim. This is one shared helper used by *every* source, so a TOML multi-line description and a wrapped README paragraph come out the same shape.

Then **cap** at `MAX_STATEMENT_CHARS` (500) **characters, not bytes**:

- If `statement.chars().count() <= 500`, keep it, `truncated = false`.
- Otherwise take the first 500 characters — via `char_indices`, never byte slicing — then, if there is a space in the last 80 characters of that slice, cut at it so a word is not split. Trim the result. `truncated = true`.

**Inline Markdown is left alone.** `**bold**` and `` `code` `` stay in the statement. Stripping emphasis is a rendering decision and this is the model; the README accuracy note says so.

If the normalized statement is empty, the README is a miss and the next source is tried.

## Reason mapping

Each row is reachable and gets a test.

| Condition | Result |
| --- | --- |
| `read_profile` is false | `NotEvaluated(Disabled)` |
| Every source missed | `NotEvaluated(InputUnavailable("no manifest description, README paragraph, or Git description found"))` |
| A source yielded a statement | `Ran(ProjectPurpose { .. })` |

There is deliberately **no `Failed` variant for this analysis**. A malformed source is a skip plus a warning, not a failure of the purpose analysis, because another source may still answer the question. That is the difference between this analysis and 4c's, and it is what criterion 6 asks for.

Warning shape, for a manifest that exists but could not be parsed or read:

```rust
ScanWarning {
    path: root.join("Cargo.toml"),   // absolute, matching every existing warning
    message: "could not be parsed for a purpose statement".to_owned(),
}
```

The message is a tool-authored constant. **Nothing from the parser, nothing from the file.** An oversized or non-UTF-8 manifest gets the same treatment with `"could not be read for a purpose statement"`.

A missing manifest produces **no warning** — most repositories are not polyglot and a warning per absent ecosystem would be noise. Only a file that *exists* and cannot be used warns.

## Files

| File | Change |
| --- | --- |
| `src/analysis/mod.rs` | `pub mod profile;`. Add `Confidence` and `pub(crate) fn read_bounded`. Module doc gains a sentence on the shared seam. |
| `src/analysis/cargo.rs` | Delete the private `read_bounded`; call `super::read_bounded(path, label, MAX_FILE_BYTES)`. Make `MAX_FILE_BYTES` `pub(crate)`. **No other change. No test change.** |
| `src/analysis/profile.rs` | New. Everything above, plus unit tests 1–16. |
| `src/lib.rs` | Re-export `Confidence`, `ProjectPurpose`, `PurposeSource`, `PURPOSE_TABLE_VERSION`, `MAX_STATEMENT_CHARS`. One `ScanReport` field, one `ScanConfig` field (+ its `Default`). Call `analysis::profile::analyze` in `scan` and extend `report.warnings`. |
| `src/render/text.rs` | A `Profile:` section, placed **before** `Git:`. Sanitize the statement and the evidence path. |
| `src/render/json.rs` | `purpose` and `purpose_table_version` fields. Additive within schema version 1. Extend the field-list test. |
| `src/render/html/mod.rs` | A Profile panel, everything escaped. |
| `src/main.rs` | `--no-profile`, wired to `read_profile`. Help text. A parser unit test. |
| `tests/common/mod.rs` | `profile_fixture_hostile_description()` and `profile_fixture_readme_only()`. |
| `tests/safety_invariants.rs` | Tests 20 and 21. Add `--no-profile` **and the currently-missing `--no-cargo`** to `every_invocation`. |
| `tests/structured_output.rs` | Tests 17, 18, 19. |
| `docs/specs/014-project-profile.md` | Mark the 6a row `Delivered`. Leave `Status: Planned` — 6b and 6c are outstanding. |
| `docs/ROADMAP.md` | Phase 6 status → `In progress (6a delivered)`. |
| `README.md` | Features, Usage, JSON output, and Roadmap. Plus the accuracy note: which manifests are read, that `.csproj` is not, the 500-character cap, and that inline Markdown is preserved. |
| `docs/build-sheets/README.md` | Index row for this sheet. |

`docs/ENGINEERING.md` needs **no** change: no new dependency, and the module tree already anticipates `analysis/*` growing.

## Tests

### Unit, in `src/analysis/profile.rs`

1. `manifest_description_wins_over_readme` — both present; assert `source == Manifest`, `evidence == Path::new("Cargo.toml")`, `confidence == Certain`. (AC 1, 2)
2. `manifest_precedence_follows_the_table` — no `Cargo.toml`, but `package.json` and `pyproject.toml` both with descriptions; `package.json` wins.
3. `reads_a_description_from_every_supported_manifest` — six cases: Cargo, `package.json`, `pyproject.toml` `[project]`, `pyproject.toml` `[tool.poetry]`, `composer.json`, `go.mod`'s module path. Each asserts statement, evidence, and `Certain`.
4. `a_manifest_without_a_description_falls_through` — `Cargo.toml` with only `name`, plus a README with prose. Result is the README. This is `Fixture::typical()`'s shape; get it right.
5. `a_workspace_inherited_description_falls_through` — `description.workspace = true` is a miss, not a crash and not a stringified table.
6. `readme_paragraph_skips_comments_headings_and_badges` — **AC 8.** A README opening with an HTML comment, an H1, a Setext underline, three badge lines, a horizontal rule, a blank line, then prose. Assert the statement is exactly the prose.
7. `readme_paragraph_skips_fenced_code_and_html_blocks` — a fenced block and a `<p>` block before the prose.
8. `readme_paragraph_joins_wrapped_lines_and_collapses_whitespace` — three wrapped lines with ragged spacing become one single-spaced sentence.
9. `statement_is_capped_on_a_character_boundary` — a paragraph of ~2000 multi-byte characters (use `é` or `日`). Assert `chars().count() <= 500`, `truncated == true`, and that the statement is valid UTF-8 that round-trips. A byte-slice implementation panics here; that is the point of the test.
10. `a_short_statement_is_not_marked_truncated` — the negative half of 9.
11. `a_readme_with_no_prose_falls_through` — title and badges only; falls to `.git/description` or `NotEvaluated`.
12. `readme_candidate_order_is_deterministic` — `README.md` and `README.txt` both present, with different content; `.md` wins, and the assertion names it.
13. `git_description_placeholder_is_ignored` — `.git/description` containing Git's default text, nothing else available; result is `NotEvaluated`.
14. `git_description_is_used_and_is_inferred` — a real description; assert `GitDescription`, `Inferred`, evidence `.git/description`.
15. `no_source_reports_not_evaluated_with_a_reason` — **AC 5.** Empty directory; `NotEvaluated(InputUnavailable(..))` whose detail names the sources looked for.
16. `a_malformed_manifest_warns_and_falls_through` — **AC 6.** A `Cargo.toml` that is invalid TOML plus a README with prose. Assert: the purpose is the README's; exactly one warning; the warning names `Cargo.toml`; and the message contains **none** of `API_KEY`, `sk_live`, or `\u{1b}`. Use the same hostile manifest bytes `tests/common/mod.rs::cargo_fixture_malformed` uses, so the leak channel under test is the one 4c measured.

Also assert somewhere in 15/16 that `read_profile: false` gives `NotEvaluated(Disabled)`, or add a seventeenth unit test for it — either is fine, but the case must be covered.

### Integration

All inside `assert_target_unchanged`.

17. `purpose_appears_in_json` — run against a fixture with a described manifest. Assert `purpose.evaluated == true`, `statement`, `evidence`, `source == "manifest"`, `confidence == "certain"`, `truncated == false`, and that `purpose_table_version` is a number.
18. `no_profile_flag_reports_not_evaluated` — `reason == "disabled"`, `statement == ""`, `source == "undetermined"`, `confidence == "inferred"`.
19. `a_repository_with_no_stated_purpose_still_exits_zero` — **AC 5** end to end. Exit 0, `purpose.evaluated == false`, `reason == "input_unavailable"`.
20. `i4_hostile_purpose_statement_does_not_reach_output_unsanitized` — a `Cargo.toml` whose `description` carries an ANSI escape. Assert no `\u{1b}` byte in `--format text` output, and none in `--format html` output either. Write the escape as TOML's `` string escape, for the reason `cargo_fixture_hostile_dependency_name` documents.
21. `malformed_manifest_does_not_abort_the_profile` — **AC 6** end to end, via `cargo_fixture_malformed()`. Exit 0; `purpose` still evaluated (that fixture has `README.md`, so give it prose — adjust the helper or build a local fixture, do not weaken the assertion); `cargo_manifest.evaluated == false` as 4c already requires; a warning present.

### Fixture helpers

```rust
/// A fixture whose `Cargo.toml` description carries an ANSI escape sequence.
/// A description is untrusted manifest content the same way a file name is;
/// used to prove the text and HTML renderers sanitize it (spec 000, I4).
pub fn profile_fixture_hostile_description() -> Fixture;

/// A fixture with no manifest description, whose README opens with badges and
/// a heading before its first real paragraph. The README-extraction path,
/// end to end (spec 014, criterion 8).
pub fn profile_fixture_readme_only() -> Fixture;
```

## Rendering

### Text — a `Profile:` section, before `Git:`

Ran:

```text
Profile:
  Purpose:  A fast, local repository summary tool for learning Rust
  Evidence: Cargo.toml (manifest, certain)
```

Truncated adds a marker the user can see:

```text
  Purpose:  Lorem ipsum ... dolor sit (truncated)
```

Not evaluated:

```text
Profile:
  Purpose:  not evaluated (no manifest description, README paragraph, or Git description found)
```

Use the existing `not_evaluated_text` helper. Sanitize `statement` with `sanitize_for_terminal` and `evidence` with `display_path`. `source.label()` and `confidence.label()` are tool-authored constants and need no sanitizing.

### HTML

A `Profile` panel following the existing `Html::push_static` / `push_escaped` idiom, in the same grid as `Git` and `Cargo`. Statement and evidence go through `push_escaped`. Nothing new in `style.css`.

### JSON

```rust
    purpose_table_version: u32,
    purpose: &'a Analysis<ProjectPurpose>,
```

Placed next to `language_table_version` / `by_language`, mirroring that pairing. Add both names to the field list in `json_render_is_valid_and_versioned`.

## Gotchas

1. **`Fixture::typical()`'s `README.md` is `# Fixture\n` — a heading and nothing else.** Under the rules above, that README yields no paragraph, its `Cargo.toml` has no `description`, and there is no `.git`. So `typical()` reports `NotEvaluated(InputUnavailable)`. That is correct, it is the AC 5 fixture, and any test that expects a purpose from `typical()` is a test written against the wrong fixture.
2. **Character cap, not byte cap.** `&statement[..500]` panics on a multi-byte boundary. Test 9 exists to catch exactly that; do not make it pass by shrinking the fixture.
3. **The badge scanner must terminate.** An unclosed `](` or a line of a thousand `[`s is untrusted input. Single forward pass, no backtracking, no `loop` without a guaranteed advance. I9.
4. **Moving `read_bounded` must change no behavior.** The four detail strings stay byte-identical and every existing `cargo.rs` test passes untouched. If one needs editing, the move was wrong — stop and report.
5. **No parser text anywhere.** Not `toml::de::Error`, not `serde_json::Error`, not `Display`, not `message()`. In this parcel nothing at all is read off an error value — the warning is a constant.
6. **A missing manifest does not warn; an unusable one does.** Warning on every absent ecosystem would put four warnings on every ordinary repository.
7. **Do not follow symlinks in the README lookup.** `file_type().is_file()` is false for a symlink, which is the behavior you want. I8.
8. **Sanitize on render, not on extraction.** Same rule as 4c. The JSON contract carries raw values; JSON string encoding already makes them safe.
9. **`Eq` must stay derivable.** Every field is `String`, `PathBuf`, `bool`, or a C-like enum. No floats.
10. **Existing output stays byte-identical apart from the additions.** `tests/cli.rs` passes unmodified. Text output gains one section; no existing line changes.
11. **Warnings are appended after traversal**, so scan-order warnings keep their existing relative order and the profile's come last. Deterministic.
12. **`--no-cargo` is missing from `every_invocation` in `tests/safety_invariants.rs`.** Add it alongside `--no-profile`; it is an existing gap this parcel is well placed to close.
13. **`PurposeSource::Undetermined` is never constructed by extraction.** If a code path can produce a `Ran(ProjectPurpose)` whose source is `Undetermined`, that path is a bug.

## Out of scope

- **The tech stack detector table** — languages by source bytes with exclusions is 6b, and runtimes, package managers, frameworks, test and build systems, containers, CI, databases, and linting are 6c. **Do not start it.** Detecting "this is a Rust project" from the `Cargo.toml` you just opened is tempting and belongs to 6c's table, not to a purpose extractor.
- **`*.csproj`.** Needs XML. Decided against in spec 014's Clarifications.
- **Workspace-inherited descriptions.** `description.workspace = true` falls through, and reading the workspace manifest to resolve it is a second-manifest traversal this parcel does not do.
- **Any network lookup**, including resolving a `go.mod` module path to a repository. I6.
- **Stripping or rendering Markdown.** The statement keeps its inline markup. A Markdown renderer is not in this project.
- **The hand-written crate error enum.** Still not forced: every failure here degrades to `NotEvaluated` or a `ScanWarning`, so `scan`'s `io::Result` is untouched. Raise it on its own merits, not as a prediction.

## Green bar

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Plus, reported in the receipt:

```bash
# This repository declares a description, so the manifest path should win.
cargo run -- . --format json | jq -S '.purpose, .purpose_table_version'

# Disabled.
cargo run -- . --format json --no-profile | jq -S '.purpose'

# A directory with nothing to state must still exit 0.
mkdir -p /tmp/rr-nopurpose && cargo run -- /tmp/rr-nopurpose --format json | jq -S '.purpose'; echo "exit=$?"

# The README path, with this repository's own README, by hiding the manifest field.
# (Use a temporary copy OUTSIDE this repository — never edit the tree you are in.)

cargo run -- . --format text | head -20

# MSRV. Stop and report if this fails; do not raise rust-version.
cargo +1.88.0 check --all-targets --all-features
```

## Definition of done

- `ScanReport` carries `purpose: Analysis<ProjectPurpose>`, and `ScanConfig` carries `read_profile`.
- All six supported manifests, the README path, and `.git/description` each produce a statement, with the evidence path and the confidence criteria 1 and 2 require.
- README extraction skips HTML comments, headings, Setext underlines, horizontal rules, fences, HTML blocks, table rows, and badge-only lines (AC 8), and the cap is enforced on character boundaries.
- A repository with no stated purpose exits 0 and reports `not evaluated` with a reason (AC 5).
- A malformed manifest warns, falls through, and does not abort the profile or the scan (AC 6), with **no parser text in the warning** — test 16 asserts the absence directly.
- The statement is sanitized in the text and HTML renderers, proven by test 20.
- `read_bounded` and `Confidence` live in `src/analysis/mod.rs`; `cargo.rs` uses the shared reader with **no test changes**.
- `--no-profile` and `--no-cargo` are both in `every_invocation`.
- `tests/cli.rs` unmodified.
- Spec 014's 6a row reads `Delivered`; `Status:` stays `Planned`.
- ROADMAP phase 6 → `In progress (6a delivered)`.
- README documents the fields, the flag, and the accuracy limits.
- `cargo +1.88.0 check` clean with `rust-version` unchanged.
- Green bar clean. No commit, no push.
