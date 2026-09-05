# Dogfood log

Repo Radar run against Repo Radar, recorded at each phase boundary.

[025 practice assessment](../specs/025-practice-assessment.md) commits this repository to being its own first fixture, and [ENGINEERING.md](../ENGINEERING.md) states the rules it will be assessed against. This directory is where the honest result is kept — including when it is unflattering, which is the only version worth keeping.

The practice is deliberately manual until phase 24, when `repo-radar practices .` runs in CI and produces these numbers itself. Until then a phase boundary means: run the tool, record the output, and record what it says about us.

## Baselines

| Phase | Date | Files | Bytes | Lines | Notes |
| --- | --- | --- | --- | --- | --- |
| 4 (parcel 4a) | 2026-09-05 | 49 | 272.5 KiB | 5,698 | First run with language, line, and directory signals. Two findings against ourselves — see below. |
| 5 (parcel 5a) | 2026-09-05 | 57 | — | — | Finding 1 partly resolved: `main.rs` 483 → 208. |

## Phase 4 — 2026-09-05

Composition:

```
Languages:
  Markdown          37 files  194.4 KiB        3250 lines
  Rust               8 files   74.9 KiB        2305 lines
  Lockfile           1 files    2.6 KiB         107 lines
  YAML               1 files      411 B          21 lines
  TOML               1 files      283 B          14 lines
  [no extension]     1 files        8 B           1 lines
```

### Finding 1 — our largest module is nearly twice our own threshold

[ENGINEERING.md](../ENGINEERING.md) sets a module-length threshold of 400 lines, chosen as roughly the point past which a file stops being readable in one sitting.

| File | Lines | Against a 400-line threshold |
| --- | --- | --- |
| `src/lib.rs` | 766 | 1.9× |
| `src/main.rs` | 483 | 1.2× |
| `src/languages.rs` | 86 | within |

`src/lib.rs` is also the largest file in the repository by bytes, ahead of every specification document.

This is the phase 5 refactor, and the number moves it from tidy-up to overdue. The target module tree in ENGINEERING.md splits traversal, per-file analysis, and rendering, which resolves both files at once. Recording it here rather than quietly fixing it, because a threshold that gets loosened the first time it bites is not a threshold.

The honest caveat: 369 of `lib.rs`'s 766 lines are its test module. Excluding tests it sits at 397, just inside. The threshold as written counts whole files, and whether it should count test modules separately is a real question — but it is one to answer in the rule, before the next reading, not after seeing a result we would prefer.

### Finding 2 — this project is 2.6× more documentation than code

199 KB of Markdown against 77 KB of Rust; 37 documents against 8 source files.

For a spec-driven project at phase 4 of 29 this is defensible — the specifications *are* the work so far, and they were written ahead of the code deliberately. It is recorded because the ratio is worth watching rather than because it is currently wrong. If it still reads 2.6:1 at phase 15, the plan has outrun the build.

### What the tool could not tell us

Everything else. There is no symbol index, no dependency graph, no coupling measure, no test-posture signal, and no practice assessment yet, so nothing here speaks to structure or coupling — only to size. That gap is the point of running this now: the baseline exists so later phases can be measured against it rather than described.

## Phase 5a — 2026-09-05 — finding 1, partly resolved

The render module split moved all three renderers out of the binary and into `src/render/`.

| File | Before | After | Against a 400-line threshold |
| --- | --- | --- | --- |
| `src/main.rs` | 483 | 208 | within |
| `src/lib.rs` | 766 | 767 | still 1.9× |
| `src/render/html/mod.rs` | — | 152 | within |
| `src/render/text.rs` | — | 121 | within |
| `src/render/html/markup.rs` | — | 95 | within |
| `src/render/json.rs` | — | 73 | within |
| `src/render/mod.rs` | — | 65 | within |

`main.rs` is resolved. **`lib.rs` is not**, and is now the only file in the repository over the threshold. It still holds traversal, the model, line counting, and directory aggregation in one file. The `scan/` and `analysis/` split from [ENGINEERING.md](../ENGINEERING.md) is what closes it, and it remains open debt rather than being quietly re-scoped.

The prompt for this parcel was a human reading `print_html` and calling it unmodular — not the tool, and not the log. Worth recording honestly: at phase 5 our own instrument measures size and nothing else, so it could report that `main.rs` was long but not that its contents were in the wrong crate. Coupling and structure are [025](../specs/025-practice-assessment.md), phase 24. Until then the log records what a person noticed, and that is a limitation of the tool rather than of the practice.
