# Feature Specification: Symbol Index

Status: Planned
Priority: P1
Depends on: `001-scan-engine`, `002-structured-output`, `007-parallel-scanning`

## Goal

Answer "what is defined in this repository and where" without requiring a language server or a compiler.

## Behavior

Extract top-level declarations per supported language into a versioned symbol table:

- Rust: `fn`, `struct`, `enum`, `trait`, `impl`, `mod`, `type`, `const`, `static`
- TypeScript and JavaScript: `function`, `class`, `interface`, `type`, exported `const`
- Python: `def`, `class`

Each symbol records kind, name, relative path, 1-based line number, and whether it is publicly exported where the language expresses that.

CLI:

```text
repo-radar symbols [PATH] [--kind KIND]... [--lang LANG]... [--format text|json]
```

The report also summarizes symbol counts per kind, per language, and the files with the most declarations.

## Acceptance Criteria

1. A fixture per supported language yields the expected symbol names, kinds, and line numbers.
2. Declarations inside string literals, line comments, and block comments are not reported.
3. Unsupported file types are skipped and counted as unparsed rather than producing a warning per file.
4. Extraction failures degrade to a per-file warning and never abort the scan.
5. `--kind` and `--lang` filters compose and an unknown value is a usage error with exit status `2`.
6. Symbol ordering is deterministic: path, then line, then name.
7. Extraction throughput is measured by a benchmark on a fixture of at least 1,000 files.

## Constraints

- Parsing is line-oriented and heuristic in this phase. Full grammars, macro expansion, and type resolution are explicitly out of scope.
- The language table is versioned data, not scattered literals, so adding a language does not require touching the traversal code.
- Accuracy limits must be documented in the README rather than hidden.
