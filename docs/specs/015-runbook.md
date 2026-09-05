# Feature Specification: Runbook Extraction

Status: Planned
Priority: P0
Depends on: `014-project-profile`

## Goal

Answer "how do I build, run, test, and configure this" so a developer can go from clone to running in one step.

## Behavior

```text
repo-radar run [PATH] [--format text|json]
```

Collect executable knowledge from the repository:

- Task definitions: `package.json` scripts, `Makefile` targets, `justfile` recipes, `Taskfile` tasks, Cargo aliases in `.cargo/config.toml`, and `pyproject.toml` script entries
- Binary and entry-point targets: `[[bin]]` in `Cargo.toml`, `src/main.rs`, `main` in `package.json`, `if __name__ == "__main__"`, `func main()`, container `ENTRYPOINT` and `CMD`
- Required configuration: variables named in `.env.example`, `.env.sample`, compose `environment` blocks, and CI workflow `env` blocks
- Declared ports from compose files, Dockerfile `EXPOSE`, and common configuration keys
- Service dependencies from compose services, so the user learns a database is required before the app fails to start
- Prerequisite tool versions from the project profile
- Documented commands extracted from README fenced shell blocks, attributed to the README

Tasks are classified into intents — `build`, `test`, `run`, `lint`, `format`, `migrate`, `deploy`, `other` — and each records its name, the raw command, its source file, and its intent.

The report proposes an ordered quick-start: install prerequisites, install dependencies, configure environment, run tests, run the application. Each step cites the evidence it came from, or is marked as unavailable.

## Acceptance Criteria

1. Every reported command records the file it came from, and its raw text is preserved exactly.
2. `.env` files that are not example templates are never read, and their values never appear in output.
3. Secret-looking values in example files are reported as key names only, never as values.
4. A fixture with `package.json` scripts, a `Makefile`, and a `justfile` reports tasks from all three without merging or deduplicating away distinct commands.
5. Intent classification is table-driven and an unmatched task is classified as `other` rather than dropped.
6. A repository with no task definitions produces a successful report stating that no tasks were found.
7. Malformed manifests and unparseable compose files produce warnings and do not abort extraction.
8. Repo Radar never executes any extracted command.

## Constraints

- This feature reads and reports commands. Executing them is permanently out of scope, because running untrusted code from a cloned repository is exactly the risk this tool exists to help the user avoid.
- Extracted command text is untrusted repository content. It is displayed as inert text, never interpolated into a shell, and terminal control sequences are stripped before display.
