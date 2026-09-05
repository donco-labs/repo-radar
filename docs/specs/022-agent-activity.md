# Feature Specification: Agent Activity

Status: Planned
Priority: P0
Depends on: `001-scan-engine`, `002-structured-output`, `004-watch-mode`, `013-provenance`

## Goal

Answer "who has been changing this, how, and how fast" when the author is an AI coding agent.

Every other analysis in Repo Radar describes a repository's *state*. This one describes its *authorship process*. Most non-trivial code is now written with agentic assistance, and that process leaves structured local traces that no repository-statistics tool reads. Reading them is the thing Repo Radar can do that `cloc`, `tokei`, `scc`, `git-quick-stats`, and the GitHub insights tab cannot.

## Scope and Consent

This analysis reads files **outside** the scanned repository — agent session logs in the user's home directory. That is a meaningful widening of what the tool touches, so:

- It is opt-in per invocation via `--agents`, and never runs by default.
- It reads only sessions whose recorded working directory resolves inside the scanned repository.
- The content is treated exactly as repository content is: local, never transmitted, never logged elsewhere.
- Prompt text and model output are **not** ingested. Only structural events are: timestamps, tool names, file paths, session boundaries, and outcomes. The lens reports what an agent *did*, not what anyone said.
- When no adapter finds a source, the analysis reports `not evaluated` with the reason, per the honesty requirements of `SPEC.md`.

## Model

The lens defines one vendor-neutral event type. Adapters translate a vendor's log format into it; nothing downstream knows which agent produced an event.

```text
AgentEvent {
    session:    SessionId,
    agent:      AgentKind,          // ClaudeCode | Cursor | Aider | Codex | Other(String)
    at:         Timestamp,
    kind:       EventKind,
    paths:      Vec<PathBuf>,       // repository-relative, may be empty
    evidence:   Evidence,           // source file and line offset
}

EventKind {
    SessionStart { cwd, branch, tool_version },
    SessionEnd,
    TurnStart,                      // a unit of user-initiated work
    Read { path },
    Write { path, kind: Create | Modify | Delete | Rename },
    Command { name },               // a shell or tool invocation, name only
    Unknown { discriminant },        // preserved so coverage is measurable
}
```

`Evidence` is mandatory on every event: the adapter source path and the offset within it. An event that cannot name its evidence is a defect, matching the `SPEC.md` output contract.

### Adapters

An adapter is a trait implementation that discovers candidate sources and yields `AgentEvent`s. Adding an agent requires no change to the model, the aggregation, or any surface.

```text
trait AgentAdapter {
    fn kind(&self) -> AgentKind;
    fn discover(&self, repository: &Path) -> Vec<SourceRef>;
    fn parse(&self, source: &SourceRef) -> Result<Vec<AgentEvent>, AdapterError>;
}
```

| Adapter | Source | Status |
| --- | --- | --- |
| Claude Code | `~/.claude/projects/<path-slug>/*.jsonl` | Implemented in this phase |
| Cursor | workspace storage database | Registered, unimplemented |
| Aider | `.aider.chat.history.md`, `.aider.input.history` | Registered, unimplemented |
| Codex | session log directory | Registered, unimplemented |

Unimplemented adapters are registered and report `unsupported` with a reason. They are not silently absent, so the report distinguishes "this agent was not used here" from "we cannot read this agent".

Every log format here is undocumented and vendor-controlled. Adapters are therefore best-effort and version-tolerant by construction: an unrecognized record becomes `Unknown` rather than an error, and each adapter reports a **coverage ratio** — recognized records over total records — so a silently drifted format is visible as falling coverage rather than as a confidently wrong report.

## Derived Analyses

From the event stream, correlated with the scan model and Git history:

- **Authorship split** — files and bytes touched in agent sessions versus files with no agent event, over a window
- **Session timeline** — sessions, their durations, and the files each touched
- **Edit velocity** — writes per minute, and the distribution across a session
- **Rework rate** — a file written N times within one session. Repeated rewriting is a signal of a fight with the code, and the heuristic is labelled as one.
- **Read-before-write ratio** — writes to files the agent never read in the session, per session and per file
- **Blind-radius** — files that depend on a changed file and were neither read nor written. Requires [010](010-dependency-graph.md); reported as unavailable until then.
- **Untested change** — source files written with no test file written in the same session, using the test-surface signals of [008](008-code-annotations.md)
- **Prompt-to-commit correlation** — turn boundaries aligned against commit timestamps, giving commits per turn and unattributed commits
- **Live pane** — under `serve`, the current session's events as they arrive, so a running agent is visible while it works

Each analysis names its evidence and states whether it ran.

## Behavior

```text
repo-radar activity [PATH] --agents [--since DURATION] [--agent NAME]
repo-radar serve [PATH] --agents
```

`--agents` enables the lens on `activity` (spec 021), on `serve` (spec 006), and on the JSON contract as an additive section. `--since` bounds the window, defaulting to 30 days. `--agent NAME` restricts to one adapter.

Under `serve`, the lens participates in the watch loop of [004](004-watch-mode.md): a new record appended to a live session log is an update event, so an agent working in the repository is observable in real time rather than after the fact.

## Acceptance Criteria

1. `--agents` is required. Without it, no path outside the scanned repository is opened, asserted by a test.
2. A session whose recorded working directory is outside the scanned repository is excluded.
3. No prompt text or model output appears in any output format, asserted by a fixture containing a distinctive string in a prompt and absent from every emitted document.
4. An unrecognized record becomes `Unknown` and lowers the coverage ratio; it never aborts the parse.
5. A truncated final line, expected in a log being appended to live, parses the preceding records successfully.
6. A registered but unimplemented adapter reports `unsupported` with a reason, distinct from "no sessions found".
7. With no agent logs present, the report succeeds and states `not evaluated` with the reason.
8. Every emitted event carries a resolvable evidence source and offset.
9. Rework rate, blind-radius, and read-before-write are each labelled as heuristics and carry their inputs.
10. Paths are reported repository-relative; a session touching files outside the repository reports those as an excluded count, not as paths.
11. Under `serve --agents`, appending records to a live session log produces a stream update within one debounce window.
12. The spec 000 immutability harness passes with `--agents` active, and a second harness asserts the agent log sources are also unmodified.
13. Fixtures cover: no logs, a live truncated log, an unknown schema version, a session in a different repository, and a multi-session repository.

## Constraints

- The lens never executes anything it finds in a log, including recorded commands. Command records contribute a name only.
- Agent logs are untrusted input, on the same footing as repository content: escaped on output, never interpolated into a shell command, never used to construct a filesystem path without validation against the repository root.
- No network access, under any flag. This analysis is entirely local.
- Adapter source locations are versioned data, not hard-coded traversal logic.
