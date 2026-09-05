# Build sheet: Render module split — parcel 5a

Date: 2026-09-05
Spec: [ENGINEERING.md](../ENGINEERING.md), roadmap phase 5
Branch: `refactor/render-module-split`

## Goal

Move rendering out of the binary and into the library, and make HTML escaping a property of the type system rather than a thing every call site must remember.

**No observable behavior change.** Every existing test must pass unmodified. If a test needs editing to go green, the refactor is wrong — stop and report.

Phase 5 splits in two. This is **5a**: module structure and the `Html` type. Parcel **5b** later adds `Analysis<T>`, the crate lints, and the declared MSRV. Do not do 5b's work here.

## Why

Three concrete failures in the current layout, all of which get worse if left:

1. **Renderers live in `src/main.rs`, a `bin` target.** No other crate or target can import them. Spec 006 `serve` (phase 11) and spec 024's view layer (phase 12) would each have to grow their own renderer, forking presentation the way `SPEC.md` forbids forking the model.
2. **Escaping is a convention.** `escape_html` must be remembered at every interpolation. One omission is an injection defect in a tool built to handle untrusted repository content. Invariant I4 deserves a compile-time guarantee, not vigilance.
3. **Renderers are untestable except through a subprocess**, because a `bin` has no importable surface.

## Target layout

```text
src/
  lib.rs              Public API and the report model. Adds `pub mod render;`.
  languages.rs        Unchanged.
  render/
    mod.rs            Re-exports; the shared `format_bytes`.
    text.rs           Human summary.
    json.rs           The versioned JSON contract.
    html/
      mod.rs          Document assembly.
      markup.rs       The `Html` type.
      style.css       The stylesheet, as a real file.
  main.rs             Argument parsing, help, dispatch. Nothing else.
```

`scan/`, `analysis/`, and `sanitize.rs` from the ENGINEERING.md target tree are **out of scope** for this parcel. Traversal stays where it is in `lib.rs`. Only rendering moves.

## Seam contracts

### `src/render/html/markup.rs`

The centrepiece. Escaping becomes unforgeable.

```rust
/// A fragment of HTML that is safe to emit.
///
/// The only route from untrusted text to `Html` is `Html::escape`, which
/// escapes on construction. Program-authored markup enters through
/// `Html::from_static`, which requires a `&'static str` — a value derived
/// from repository content is never `'static`, so it cannot take that path.
///
/// This makes invariant I4 a property the compiler checks rather than a rule
/// every call site must remember.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Html(String);

impl Html {
    /// Escapes untrusted text. The only constructor that accepts a borrowed
    /// non-static string.
    pub fn escape(text: &str) -> Self;

    /// Markup the program itself authored. Never reachable from repository
    /// content, because such a value cannot be `'static`.
    pub fn from_static(markup: &'static str) -> Self;

    /// A number. Always safe; no escaping is possible or needed.
    pub fn number(value: u64) -> Self;

    pub fn push(&mut self, other: &Html);
    pub fn push_static(&mut self, markup: &'static str);
    pub fn push_escaped(&mut self, text: &str);

    pub fn as_str(&self) -> &str;
    pub fn is_empty(&self) -> bool;
}

impl fmt::Display for Html;
```

`Html::escape` must produce byte-identical output to the current `escape_html`: `&` → `&amp;`, `<` → `&lt;`, `>` → `&gt;`, `"` → `&quot;`, in that precedence. Move the existing implementation; do not rewrite it.

**`style.css` is loaded with `include_str!` and passed through `Html::from_static`.** Extract the current CSS verbatim into that file and undouble the braces — they were doubled only to survive `format!`, and a real file needs no escaping.

### `src/render/mod.rs`

Every renderer writes into a caller-supplied sink and returns the sink's error, so the same code serves `stdout` today and a `TcpStream` at phase 11.

```rust
pub mod html;
pub mod json;
pub mod text;

/// Formats a byte count as B, KiB, MiB, or GiB.
pub fn format_bytes(bytes: u64) -> String;
```

### Renderer signatures

```rust
// render/text.rs
pub fn write_summary(out: &mut impl fmt::Write, root: &Path, report: &ScanReport) -> fmt::Result;

// render/json.rs
pub fn write_json(out: &mut impl fmt::Write, root: &Path, report: &ScanReport) -> fmt::Result;

// render/html/mod.rs
pub fn write_html(out: &mut impl fmt::Write, root: &Path, report: &ScanReport) -> fmt::Result;
```

`fmt::Write` rather than `io::Write`: it is what `String` implements, which is what makes these unit-testable without a process or a socket. Phase 11 adapts at its own boundary.

**Every `let _ = write!(…)` disappears.** Propagate with `?`. A discarded `Result` here is the exact pattern that hides a real failure once the sink is a socket.

`JsonReport` moves to `render/json.rs` and stays private to it.

### `src/main.rs` after the move

Retains, and nothing else: `OutputFormat`, `Options`, `Default for Options`, `main`, `parse_arguments`, `take_value`, `print_help`, and the argument-parsing tests.

Dispatch becomes:

```rust
let mut output = String::new();
let result = match options.format {
    OutputFormat::Text => render::text::write_summary(&mut output, &options.root, &report),
    OutputFormat::Json => render::json::write_json(&mut output, &options.root, &report),
    OutputFormat::Html => render::html::write_html(&mut output, &options.root, &report),
};
result.expect("writing to a String cannot fail");
print!("{output}");
```

The `expect` states the invariant it relies on, which is the documented exception in ENGINEERING.md's panic policy.

## Byte-identical output

This is the parcel's controlling constraint.

- `print_summary` ended each section with `println!`, so the text output ends in a newline. `write_summary` must reproduce that exactly.
- `print_json` used `println!`, so JSON output ends in a newline. Preserve it.
- `print_html` used `print!` with no trailing newline. Preserve that too.
- Field order in `JsonReport` is the serialized order. Do not reorder.
- The HTML is compared by existing tests for `<!doctype html>`, `Repo Radar`, `no external assets or requests`, absence of `<script`, and absence of `https://`. All must still hold.

Verify by capturing `cargo run -- . --format <each>` before and after and diffing. Report the diff result in the receipt. A non-empty diff is a stop condition.

## Files

| File | Change |
| --- | --- |
| `src/render/mod.rs` | **New.** Module declarations, `format_bytes` moved from `main.rs` with its test. |
| `src/render/text.rs` | **New.** `write_summary`, from `print_summary`. |
| `src/render/json.rs` | **New.** `write_json` and private `JsonReport`, from `main.rs`. |
| `src/render/html/mod.rs` | **New.** `write_html`, from `print_html`, rewritten to use `Html`. |
| `src/render/html/markup.rs` | **New.** The `Html` type, absorbing `escape_html`. |
| `src/render/html/style.css` | **New.** The stylesheet, extracted verbatim, braces undoubled. |
| `src/lib.rs` | Add `pub mod render;`. No other change. |
| `src/main.rs` | Delete the four render functions, `JsonReport`, `escape_html`, `format_bytes`; add dispatch; keep argument parsing and its tests. |

No change to `tests/`, `benches/`, or `Cargo.toml`. No new dependency.

## Tests

Renderers are importable now, so test them directly rather than through a subprocess.

1. `render::format_bytes` — move the existing `formats_bytes_for_humans` test across unchanged.
2. `Html::escape` — absorb the existing `html_escapes_repository_content` assertion: `<script>"&` becomes `&lt;script&gt;&quot;&amp;`.
3. `Html::escape` on already-escaped text does not double-escape beyond the documented behavior — assert the exact current output rather than inventing a rule.
4. `html_renders_repository_content_as_inert_text` — build a `ScanReport` with a file named `<script>evil</script>.rs`, render, assert the output contains no literal `<script>` and does contain `&lt;script&gt;`.
5. `text_summary_matches_expected_shape` — render a small fixture report into a `String` and assert the section headings and the trailing newline.
6. `json_render_is_valid_and_versioned` — render into a `String`, parse with `serde_json`, assert `version == 1` and that every field the old `JsonReport` carried is present.
7. `renderers_are_reusable_across_sinks` — render the same report into two separate `String`s and assert equality. This is the property phase 11 depends on.

Every existing test in `tests/` stays untouched and must pass.

## Gotchas

1. **`include_str!` resolves relative to the containing source file.** From `src/render/html/mod.rs`, the stylesheet is `include_str!("style.css")`.
2. **Undoubling the CSS braces is mechanical and easy to get wrong.** The literal currently contains `{{` and `}}` only because `format!` required it. In a real file every `{{` becomes `{` and every `}}` becomes `}`. Nothing else changes — not a space, not a colour.
3. **`write!` into `impl fmt::Write` needs `use std::fmt::Write`** in scope, and it collides with `io::Write` if both are imported. Import only what each module needs.
4. **`Html::from_static` must not be reachable from a `String`.** If a `format!` result needs emitting, escape it or push it as parts. Do not add a constructor that takes an owned `String` — that would reopen the hole this parcel closes.
5. **`display_path` already sanitizes for the terminal.** HTML rendering still escapes on top of it. Keep both; they defend different sinks.
6. **The `.stats` grid is `repeat(4, 1fr)`** after parcel 4a. Do not revert it to 3 while moving the CSS.
7. **Text output's trailing newline** comes from `println!`. Using `write!` without `\n` silently drops it and breaks output comparison.
8. **Do not "improve" the markup or the styling.** This parcel moves code. A visual change makes the byte-identical check impossible to interpret.

## Green bar

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Plus the before/after output diff for all three formats.

## Out of scope

- `Analysis<T>`, crate lints, MSRV — parcel 5b.
- `scan/`, `analysis/`, `sanitize.rs` modules — a later parcel.
- Git basics (4b), Cargo parsing (4c).
- Any visual or behavioral change whatsoever.

## Definition of done

- Output byte-identical for all three formats, verified by diff.
- All existing tests pass **unmodified**.
- Seven new tests above added and passing.
- No `let _ = write!` remains in the crate.
- `main.rs` contains no rendering code.
- Green bar clean. No commit, no push.
