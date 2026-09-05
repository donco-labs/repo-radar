# Feature Specification: Agentic Readiness

Status: Planned
Priority: P1
Depends on: `013-provenance`, `014-project-profile`
Deepens with: `022-agent-activity`

## Goal

Answer "is this repository set up to be worked on by agents, and how is that work governed" — from the repository's own configuration, before a single agent runs.

```text
repo-radar agentic [PATH]
```

## Why This Is Separate

Three specifications now touch agentic work and they answer different questions. Keeping them distinct matters, because conflating configuration with behavior is how a tool ends up claiming a well-configured repository is a well-run one.

| Spec | Question | Source |
| --- | --- | --- |
| **026** (this) | How is it *set up* for agents? | Repository configuration, committed files |
| [022](022-agent-activity.md) | What did agents *actually do*? | Local session logs, outside the repository |
| [025](025-practice-assessment.md) | Is the *code* built well? | Source structure and toolchain |

026 reads only files inside the repository, needs no opt-in beyond the ordinary scan, and works on a clone on a machine where no agent has ever run. 022 needs `--agents` and local logs. Together they produce the drift analysis below, which neither can produce alone.

## Dimensions

### 1. Agent surfaces

Which agent ecosystems this repository is configured for, from a versioned detector table. Presence, path, and size — never a judgement of the content.

| Ecosystem | Evidence |
| --- | --- |
| Cross-vendor | `AGENTS.md` |
| Claude Code | `CLAUDE.md` (root and nested), `.claude/` |
| Cursor | `.cursorrules`, `.cursor/rules/*.mdc` |
| GitHub Copilot | `.github/copilot-instructions.md` |
| Aider | `.aider.conf.yml`, `CONVENTIONS.md` |
| Windsurf | `.windsurfrules` |
| Cline | `.clinerules` |
| Continue | `.continue/` |
| Zed | `.rules` |

A repository with none of these reports `no agent configuration found` — a fact, not a deficiency. Plenty of good repositories have none.

### 2. Custom tooling

What the repository has built for its agents, rather than merely configured:

- **Subagents** — `.claude/agents/*.md`: name, description, declared `tools`, declared `model`
- **Skills** — `.claude/skills/*/SKILL.md`: name, trigger description
- **Slash commands** — `.claude/commands/*.md`
- **Hooks** — from `.claude/settings.json`: which events, which matchers, which commands
- **MCP servers** — from `.mcp.json`: name, transport, and **whether the command or URL is present**, never its arguments verbatim

This is the dimension that distinguishes a repository someone configured from one someone *invested in*. It is reported as counts and names with evidence paths, never as a maturity level.

### 3. Method

Whether agent work is governed by an artifact or by conversation alone. Detected from structure, each with its evidence:

| Method | Evidence |
| --- | --- |
| **Spec-driven (SDD)** | A specification directory, numbered or titled specs, specs carrying acceptance criteria, an instruction file stating a spec-first rule |
| **Plan-artifact** | Persisted plan or build-sheet documents, an index over them, plans referenced by commits |
| **ADR-driven** | `docs/adr/`, `docs/decisions/`, or numbered decision records |
| **Roadmap-gated** | A roadmap with ordered phases and status, referenced by an instruction file |
| **Conversation-only** | Agent configuration present, none of the above |

`conversation-only` is reported as a **finding of absence, not a fault**. Many projects are run well without written method, and the tool says which it found, not which is correct.

Acceptance criteria are the strongest available SDD signal: a specification directory whose documents contain numbered, testable criteria is materially different from one containing prose intentions, and the two are distinguished.

### 4. Governance

How agent authority is bounded. Read from configuration, reported as **exposure**, never as "insecure":

- **Permission breadth** — allow/deny entries in `.claude/settings.json`. A wildcard such as `Bash(*)` is reported with its exact config line and the note that it permits arbitrary command execution. The tool states what the setting permits; it does not tell the user their choice is wrong.
- **Tool restriction on subagents** — agents declaring a narrowed `tools` list versus agents inheriting everything.
- **Hook gating** — presence of `PreToolUse` / `PostToolUse` hooks, which is the only mechanism that can mechanically block an agent action.
- **CI as a gate** — whether the checks in CI are the same ones an instruction file tells agents to run. A green bar an agent is told to run but CI does not enforce is a gap worth naming.
- **Attribution convention** — an instruction file requiring a commit trailer, and whether history actually carries it.
- **Shared versus personal config** — whether `settings.local.json` is git-ignored, which is the difference between a team convention and one developer's machine.

### 5. Secret hygiene

Agent configuration files are a known place for credentials to be committed by accident: MCP server definitions carry API keys, and `settings.json` carries environment variables.

The scan reports **the file and the key name** of anything matching a credential pattern. It **never emits the value**, not in text, not in JSON, not in HTML, not in a warning, and not truncated.

This is the one dimension where the tool says something is wrong rather than merely present, because a committed live credential is not a stylistic choice.

### 6. Instruction weight

Structural facts about instruction files, with an explicit limit on what they mean:

- Size, and whether it is split across nested files
- Whether it references the specification or roadmap documents found in dimension 3
- Age of its last modification against the repository's last commit — an instruction file untouched for a year in an active repository is a staleness signal

**Repo Radar does not assess whether an instruction file is *good*.** It cannot read intent, and a short precise file beats a long vague one. Size is reported as a fact with that caveat attached, and no threshold turns it into a finding.

### 7. Traceability

From Git history, via [013](013-provenance.md):

- Share of commits carrying an agent attribution trailer
- Share of commits or pull requests referencing a spec, plan, or issue identifier
- Whether plan artifacts are committed alongside the code they produced, or only after

### 8. Configured versus used

Available only when [022](022-agent-activity.md) also ran, and marked `not evaluated` otherwise:

- Subagents declared but never invoked
- MCP servers configured but never called
- Skills defined but never triggered
- Instruction files that exist but were never read in any recorded session

This is the dimension with no equivalent anywhere else, and it is the honest one: it compares what a repository *claims* about its agentic setup against what actually happened in it. Configuration is easy to accumulate and hard to prune, and unused tooling is a maintenance cost presented as a capability.

## Honesty Requirements

Inherited from `SPEC.md` and applied at the same strictness as [025](025-practice-assessment.md):

- **No score, no grade, no maturity level.** No "agentic readiness: 7/10". A single figure over these dimensions would be invention.
- **Presence is not quality.** The tool reports that a `CLAUDE.md` exists and how large it is. It does not claim it is good, and any wording implying otherwise is a defect.
- **Absence is not a fault.** `conversation-only` and `no agent configuration found` are neutral facts.
- **Every finding cites its evidence path**, and a line where the signal is line-anchored.
- **Detector tables are versioned**, and the version appears in output, so a changed finding can be attributed to a changed rule rather than to changed code.

## Acceptance Criteria

1. A repository with no agent configuration produces a successful report stating none was found, and no dimension reports a fault.
2. Every detected surface, subagent, skill, command, hook, and MCP server names the evidence file that produced it.
3. A credential-shaped value in `.mcp.json` or `.claude/settings.json` is reported by file and key name, and **its value never appears in any output**, asserted by a fixture containing a distinctive secret string absent from every emitted document.
4. MCP server arguments are not emitted verbatim; only the server name, transport, and presence of a command or URL.
5. A wildcard permission entry is reported with its config line and a statement of what it permits, containing no pejorative.
6. Method detection distinguishes a specification directory with numbered acceptance criteria from one with prose only, and cites the document that decided it.
7. `conversation-only` and `no agent configuration found` are reported as neutral facts, asserted by a test against a documented pejorative list.
8. Dimension 8 reports `not evaluated` with the reason when [022](022-agent-activity.md) did not run.
9. Instruction-file size is reported with its stated limitation and produces no threshold finding.
10. A malformed `.mcp.json` or `settings.json` produces a warning and does not abort the report.
11. Adding an ecosystem to the detector table requires no change to traversal, aggregation, or reporting code.
12. Fixtures cover: no agent configuration, Claude Code only, multiple ecosystems, a spec-driven repository, a conversation-only repository, a committed credential, a malformed configuration file, and a repository whose declared subagents were never used.
13. The spec 000 immutability harness passes for every invocation.

## Constraints

- Configuration files are untrusted input. An instruction file, a hook definition, and an MCP server entry are repository content: escaped on output, never interpolated into a shell command, and — per invariant I3 — **never executed**. A hook command is reported as inert text, exactly as [015](015-runbook.md) treats a build command.
- Read-only and offline. The scan does not start an MCP server, invoke a hook, or contact anything a configuration names.
- Analysis lives here; rendering lives in the surfaces. Findings feed [020 brief](020-brief.md), [018 health](018-health-assessment.md), and the `serve` surface.
- This specification assesses configuration. It makes no claim about the quality of agent output — that is what [025](025-practice-assessment.md) measures on the resulting code, and what a human review measures on everything else.
