# Repo Radar

Repo Radar is a fast, local repository summary tool built as a Rust learning project.

Development is spec-first. [SPEC.md](SPEC.md) defines the product behavior, and [docs/SDD.md](docs/SDD.md) defines the development process and quality gates.

The planned feature sequence and delivery timeline are in [docs/ROADMAP.md](docs/ROADMAP.md).

## Usage

```text
cargo run -- [PATH] [--top N]
```

Examples:

```text
cargo run -- .
cargo run -- ~/dev/my-project --top 5
cargo run -- --help
```

The first milestone reports file count, total size, extension counts, and the largest files. It skips build and dependency directories (`.git`, `target`, and `node_modules`) and does not follow symbolic links.

## Learning path

1. Add structured command parsing and JSON output.
2. Add parallel directory scanning and benchmark it against the sequential implementation.
3. Add file watching for live updates.
4. Add a terminal interface with dependency and activity views.
5. Add an optional browser UI backed by a local server.