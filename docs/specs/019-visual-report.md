# Feature Specification: Visual Report

Status: Planned
Priority: P1
Depends on: `016-subsystem-map`, `017-dependency-intelligence`, `018-health-assessment`, `021-activity`

## Goal

Turn the collected model into something a person can look at and understand in a minute, without installing a viewer or starting a server.

## Behavior

```text
repo-radar report [PATH] --out FILE [--open]
```

Produce a single self-contained HTML file with no external requests, containing:

- **Header**: name, purpose, provenance, license, stack badges, and health score
- **Subsystem diagram**: the subsystem graph as an interactive SVG, nodes sized by bytes and colored by health, edges weighted by coupling, with cycles highlighted
- **Dependency view**: third-party dependencies grouped by ecosystem and consuming subsystem, marked by staleness and license class
- **Activity panel**: commit sparkline, contributor summary, and a churn-versus-size hotspot scatter plot where the risky quadrant is labelled
- **Health panel**: findings ranked by severity, each expandable to its evidence
- **Runbook panel**: the quick-start and task list from spec 015, as inert copyable text
- **File treemap**: sized by bytes, colored by language, drillable by subsystem

Also emit machine formats for other tools:

- `--format mermaid` for embedding a subsystem diagram in Markdown
- `--format dot` for Graphviz
- `--format json` for the full model

## Acceptance Criteria

1. The HTML file opens correctly from `file://` with no network access, and contains no external script, style, font, or image reference.
2. All repository-derived text is escaped on insertion; a fixture containing `<script>` in a filename, a branch name, and a commit message produces no executable content in the output.
3. The report is readable in both light and dark color schemes.
4. A repository with a missing analysis renders the remaining panels and states which are unavailable.
5. Diagram layout is deterministic, so regenerating an unchanged repository yields an identical file.
6. A graph beyond a documented node threshold degrades to aggregated subsystem view with a stated reason, rather than rendering an unreadable hairball.
7. `--out` refuses a path inside the scanned repository, upholding invariant I5 of spec 000.
8. Mermaid output parses in a standard Mermaid renderer.
9. The report states the tool version, the scan timestamp, and the repository commit it describes.
10. Generation cost is benchmarked on a repository with at least 5,000 files.

## Constraints

- Self-contained means self-contained. No content delivery network, no telemetry pixel, no font fetch, enforced by a test that greps the output for external URL schemes.
- The snapshot is **rendered by the native binary**, not by the wasm view layer of [024](024-view-layer.md). It is inert HTML and SVG with no runtime: a document that must open from `file://` on a machine that may never run the tool, years after it was generated. Embedding a wasm blob to render a frozen report would add weight and a runtime dependency to buy nothing. The two surfaces share the model, not the renderer, and the visual language should stay recognisably the same across both.
- The report is a rendering of the model from earlier specs and must contain no analysis logic of its own.
- Visual encoding must be legible without color alone, so severity and staleness also carry shape or text.
