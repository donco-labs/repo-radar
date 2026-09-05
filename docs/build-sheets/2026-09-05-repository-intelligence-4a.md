# Build sheet: Repository intelligence 4a — text signals and directory aggregates

Date: 2026-09-05
Spec: [003 repository intelligence](../specs/003-repository-intelligence.md), parcel **4a**
Branch: `feat/repository-intelligence-4a`
Phase: roadmap phase 4, first of three parcels

## Goal

Give the scan model real composition: how many lines, in which languages, and which directories carry the weight. Extension counts stay, but they stop being the only answer to "what is in here".

Parcel 4a covers acceptance criteria **1, 2, and 7** of spec 003. Criteria 3, 4, 5 belong to parcels 4b (Git) and 4c (Cargo) and are **out of scope** — do not implement them, and say so in the receipt.

## Seam contracts

Fill these exactly. Do not rename, do not add fields, do not change the ordering rules.

### `src/languages.rs` (new module)

```rust
/// Version of the extension-to-language table. Bump on any entry change.
pub const LANGUAGE_TABLE_VERSION: u32 = 1;

/// Files whose extension is not in the table.
pub const UNRECOGNIZED: &str = "[unrecognized]";

/// Files with no extension at all.
pub const NO_EXTENSION: &str = "[no extension]";

/// Resolves a lowercase, dotless extension to a language family.
pub fn language_for_extension(extension: &str) -> Option<&'static str>;
```

Backing table is a `&[(&str, &str)]` static, sorted by extension so it is scannable, looked up with a linear or binary search — the table is small and this is not the hot path. Seed it with exactly these entries and no others:

```
rs → Rust            toml → TOML          md → Markdown        json → JSON
yml → YAML           yaml → YAML          lock → Lockfile      txt → Plain text
py → Python          js → JavaScript      mjs → JavaScript     cjs → JavaScript
ts → TypeScript      tsx → TypeScript     jsx → JavaScript     go → Go
c → C                h → C                cpp → C++            hpp → C++
cc → C++             java → Java          kt → Kotlin          rb → Ruby
sh → Shell           bash → Shell         zsh → Shell          html → HTML
css → CSS            sql → SQL            xml → XML            csv → CSV
```

`language_for_extension("[no extension]")` returns `None` — the caller maps that sentinel to `NO_EXTENSION` before consulting the table. Keep that decision in the caller, not the table.

### `src/lib.rs` — model additions

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanguageStat {
    pub language: String,
    pub files: usize,
    pub bytes: u64,
    /// Zero when line counting was disabled or the files were binary.
    pub lines: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DirectoryEntry {
    /// Repository-relative. Never the root.
    pub path: PathBuf,
    /// Aggregate over the directory and all its descendants.
    pub files: usize,
    pub bytes: u64,
}

/// Line counting results, and whether the analysis ran at all.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LineCounts {
    /// False when `--no-lines` was passed. Surfaces must then say
    /// "not evaluated" rather than rendering zero (invariant I10).
    pub evaluated: bool,
    pub lines: u64,
    pub text_files: usize,
    pub binary_files: usize,
    pub unreadable_files: usize,
}
```

`ScanConfig` gains one field:

```rust
pub struct ScanConfig {
    pub ignored_directories: Vec<String>,
    /// Read file contents to count lines. Defaults to true.
    pub count_lines: bool,
}
```

`ScanReport` gains four fields, in this position:

```rust
pub struct ScanReport {
    pub files: usize,
    pub bytes: u64,
    pub by_extension: BTreeMap<String, usize>,
    pub by_language: Vec<LanguageStat>,          // NEW
    pub largest_files: Vec<FileEntry>,
    pub largest_directories: Vec<DirectoryEntry>, // NEW
    pub lines: LineCounts,                        // NEW
    pub warnings: Vec<ScanWarning>,
}
```

### Ordering rules (determinism is an invariant, not a nicety)

- `by_language`: bytes descending, then `language` ascending.
- `largest_directories`: bytes descending, then `path` ascending.
- `largest_files`: unchanged — bytes descending, then path ascending.

Accumulate directories and languages in a `BTreeMap` during traversal, then sort into the `Vec` once at the end of `scan`, beside the existing `largest_files` sort.

### Line counting

Add to `src/lib.rs`, private:

```rust
enum FileText { Text { lines: u64 }, Binary, Unreadable }

fn count_lines(path: &Path) -> FileText;
```

Algorithm, exactly:

1. Open the file. On error, return `Unreadable`.
2. Read through a `BufReader` with a **64 KiB** buffer, in chunks — never `read_to_end`, never `read_to_string`. Memory must stay bounded for a multi-gigabyte file (invariant I9).
3. While reading the **first 8 KiB only**, scan for a `0x00` byte. If found, stop reading immediately and return `Binary`.
4. Otherwise count `b'\n'` occurrences across the whole file.
5. If the file is non-empty and its final byte is not `b'\n'`, add one.
6. An empty file returns `Text { lines: 0 }`. It is text, not binary.

Do **not** validate UTF-8. The NUL heuristic is what spec 003 specifies; the doc comment on `count_lines` must say it is a heuristic and name its known false positives (UTF-16 source, a text file with a stray NUL).

`Unreadable` increments `lines.unreadable_files` and **also** pushes a `ScanWarning` naming the path, matching how the scanner already reports unreadable metadata. It never aborts the scan.

### Directory aggregation

For each counted file, take its repository-relative path and walk `Path::ancestors()`, skipping the file itself and skipping the empty root component. Add the file's bytes and a count of 1 to every ancestor, **including** the repository root.

The root is aggregated so criterion 2 has a check to assert against, then **excluded** from `largest_directories` when the sorted vector is built. Represent the root key as `PathBuf::from("")` — the natural last element of `ancestors()` — so no sentinel string is invented.

## Files to change

| File | Change |
| --- | --- |
| `src/languages.rs` | **New.** Table, version constant, `language_for_extension`, unit tests. |
| `src/lib.rs` | `mod languages` + re-export; the four model types; `ScanConfig.count_lines`; `count_lines`; directory and language accumulation in `scan_directory`; sorting in `scan`; unit tests. |
| `src/main.rs` | `--no-lines` flag; help text; text output sections; `JsonReport` fields; HTML panels; unit tests. |
| `tests/structured_output.rs` | Assert the new JSON fields and the `--no-lines` contract. |
| `tests/safety_invariants.rs` | Immutability across `--no-lines` and default line counting. |
| `benches/scan_engine.rs` | Leave alone unless it fails to compile; if it does, the minimum fix only. |

No other file changes. No new dependency — `std` and the existing `serde` only.

## CLI

Add `--no-lines`. It takes no value. Parsing lives in the existing `match` in `parse_arguments`, before the `other if other.starts_with('-')` arm:

```rust
"--no-lines" => {
    options.count_lines = false;
    index += 1;
}
```

`Options` gains `count_lines: bool`, defaulting to `true`, and `main` builds `ScanConfig { count_lines: options.count_lines, ..ScanConfig::default() }`.

Help text gains, aligned with the existing block:

```
  --no-lines            Skip line counting (faster; lines report as not evaluated)
```

`--top N` truncates `largest_directories` to the same N it already applies to `largest_files`, in `main`, not in the library. The library always returns the full sorted lists.

## Output

### Text (`print_summary`)

Header gains a lines row. When `lines.evaluated` is false, print the literal `not evaluated`:

```
Repository: .
Files:      39
Size:       144.7 KiB
Lines:      4,210            <- or "not evaluated"
```

Do not add thousands separators; print the raw integer. The column above is illustrative of position only.

Then a new section before the existing extension list:

```
Languages:
  Rust              7 files    47.3 KiB     1,505 lines
  Markdown         28 files    82.1 KiB     2,344 lines
```

Again, no separators — plain integers, columns aligned with the existing `{:<16}` / `{:>10}` style. Omit the lines column entirely when `lines.evaluated` is false.

Rename the existing `Languages / extensions:` heading to `Extensions:`. It was always extensions, and now that real languages exist the old label is wrong.

Add a `Largest directories:` section after `Largest files:`, same two-column shape as largest files, with the heading noting the figures are aggregate:

```
Largest directories (aggregate, including subdirectories):
    47.3 KiB  docs/specs
    23.9 KiB  src
```

Every path and extension goes through `display_path` / `sanitize_for_terminal` as the existing code already does. Language names come from our own static table and need no sanitizing, but do not special-case them — passing them through costs nothing and removes a question.

### JSON (`print_json`)

Additive only. **`version` stays `1`** — spec 002 permits added fields within a version, and this parcel removes and renames nothing. Add to `JsonReport`:

```rust
language_table_version: u32,      // repo_radar::LANGUAGE_TABLE_VERSION
by_language: &'a [LanguageStat],
largest_directories: &'a [DirectoryEntry],
lines: &'a LineCounts,
```

`lines` serializes with its `evaluated` flag, so a consumer can distinguish "zero lines" from "not counted". Do not omit the object when disabled.

### HTML (`print_html`)

Three changes, and no restructuring of what is there:

1. The header stat block gains a Lines figure, rendering the string `not evaluated` when `lines.evaluated` is false.
2. The existing composition-bar panel switches its source from `by_extension` to `by_language`, with bar width proportional to `bytes` rather than file count. Keep the existing markup, classes, and styling.
3. A new `Largest directories` panel, copying the `Largest files` table markup exactly, with the heading noting aggregate figures.

Every interpolated value keeps going through `escape_html`. Do not introduce a `<script>` tag, an external URL, or a font reference — `tests/structured_output.rs::html_output_is_a_self_contained_dashboard` asserts against all of them.

## Test list

Add these. Each names the criterion it covers.

### `src/languages.rs` unit tests

1. `maps_known_extensions` — `rs`→`Rust`, `yml` and `yaml` both →`YAML`, `cc`→`C++`.
2. `returns_none_for_unmapped_extensions` — `xyz` and the `[no extension]` sentinel both give `None`.

### `src/lib.rs` unit tests

3. `counts_lines_across_text_files` — two files, one with a trailing newline and one without; assert the exact total. **(AC1)**
4. `treats_nul_bearing_files_as_binary` — a fixture file containing `b"\x00\x01\x02"`; assert `binary_files == 1`, `text_files == 0`, and `lines == 0`. **(AC1)**
5. `empty_file_is_text_with_zero_lines` — asserts it is not classified binary. **(AC1)**
6. `binary_files_still_contribute_bytes_and_extension` — a binary `.bin` file still lands in `bytes` and `by_extension`. **(AC1)**
7. `directory_totals_sum_descendant_files` — `src/a.rs`, `src/nested/b.rs`, `top.rs`; assert `src` aggregates both descendants, `src/nested` only one, and that the **root aggregate equals `report.bytes`**. **(AC2)**
8. `largest_directories_excludes_the_root` — no entry has an empty path. **(AC2)**
9. `language_stats_group_and_sort_by_bytes` — Rust and Markdown files of known sizes; assert order and per-language byte totals.
10. `unmapped_extensions_group_as_unrecognized` — a `.xyz` file lands in `[unrecognized]`, and the sum of `by_language` bytes equals `report.bytes`.
11. `disabling_line_counting_reports_not_evaluated` — `ScanConfig { count_lines: false, .. }` gives `evaluated == false` and `lines == 0`. **(AC7)**
12. `unreadable_file_warns_and_counts_without_aborting` — construct if feasible on the platform; if it cannot be constructed portably, skip it and say so in the receipt rather than writing a test that does not test.

### `tests/structured_output.rs`

13. `json_carries_language_and_directory_fields` — `version` is still `1`; `by_language`, `largest_directories`, `lines`, and `language_table_version` are present and correctly typed.
14. `no_lines_flag_reports_not_evaluated_in_json` — run with `--no-lines`; assert `lines.evaluated == false`. **(AC7)**

### `tests/safety_invariants.rs`

15. Extend the existing command matrix so a default run **and** a `--no-lines` run both pass `assert_target_unchanged`. Line counting opens and reads every file in the target — that is exactly the change most likely to touch access times, so this is the parcel's most load-bearing test. **(AC7, invariant I1)**

## Green bar

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

All three must pass. Do not weaken, `#[ignore]`, or delete an existing test to reach green.

## Gotchas

1. **`ScanConfig` gains a field, and an existing test constructs it literally.** `src/lib.rs::supports_additional_ignored_directories` builds `ScanConfig { ignored_directories: vec![...] }` and will stop compiling. Fix it with `..ScanConfig::default()`. Search for every other literal construction before you build.
2. **`ScanReport` derives `PartialEq, Eq`.** Keep every new field integer or boolean. Introducing a float breaks `Eq` and the derive will fail.
3. **`ScanReport` derives `Default`.** `LineCounts::default()` gives `evaluated: false`, which is correct for a default-constructed report but means `scan` must set `evaluated = config.count_lines` explicitly. Do not rely on the default.
4. **Truncation happens in `main`, not `scan`.** `largest_files` is already truncated after the call; `largest_directories` must be truncated in the same place. A library that truncates would break the directory-sum assertion in test 7.
5. **`Path::ancestors()` ends with an empty path.** That empty entry is the repository root and is the key you aggregate under. Do not filter it during accumulation — filter it when building `largest_directories`.
6. **Read the first 8 KiB for NUL, not the whole file.** Scanning every byte of a large binary for NUL is wasted work, and the spec specifies the first 8 KiB.
7. **Stream, never slurp.** `read_to_end` or `read_to_string` on an untrusted repository is an unbounded allocation and violates invariant I9.
8. **Symlinks are already excluded** before the file branch in `scan_directory`. Do not add a second check, and do not move the line-counting call above it.
9. **Extensions are lowercased already** by the existing code. Pass that same lowercased value to `language_for_extension`; do not lowercase twice or use the raw extension.
10. **The `[no extension]` sentinel is an existing public behavior.** It stays exactly as it is in `by_extension`. In `by_language` those files group under `[no extension]` too, not under `[unrecognized]` — the two gaps are different and are reported differently.
11. **The HTML test greps for `https://` and `<script`.** Any new panel that introduces either will fail an existing test.
12. **Doc comments explain why, not what.** The security-relevant ones — the NUL heuristic, the bounded read — must state the reasoning, matching how `sanitize_for_terminal` is documented.

## Out of scope

Do not implement, and note as not done:

- Git status counts and commit activity (parcel 4b, criteria 3 and 4)
- Cargo manifest and lockfile parsing (parcel 4c, criterion 5)
- Parallel scanning (spec 007, roadmap phase 16)
- Any change to `serve`, the view layer, or agent activity

## Definition of done

- All 15 tests above written and passing, or explicitly reported as not written with the reason.
- Green bar clean on all three lanes.
- Criteria 1, 2, and 7 of spec 003 covered by a named test.
- Spec 003 `Status` **not** changed — it stays `Planned` until parcels 4b and 4c land.
- No commit, no push, no PR. Leave the work in the tree.
