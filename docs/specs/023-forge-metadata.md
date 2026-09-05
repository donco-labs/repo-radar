# Feature Specification: Forge Metadata

Status: Planned
Priority: P1
Depends on: `013-provenance`

## Goal

Answer "what is this project's standing in the world" for a repository that has a hosted counterpart. Provenance ([013](013-provenance.md)) reads the local clone and can say where the code came from. It cannot say whether the project is popular, maintained, archived, or how far the local fork has drifted from what everyone else is using. That requires asking the forge.

## Rationale

A developer opening an unfamiliar clone forms a judgement partly from social signal: a project with 20,000 stars and a release last week is a different proposition from one with three stars, archived in 2021, even when the code looks identical. That signal is the first thing a person checks and the last thing a local tool can derive.

## Network Policy

This is the only specification in Repo Radar that permits outbound network access, and it inherits the constraints of `SPEC.md` without relaxation:

- Access is opt-in per invocation via `--network`, and is never implied by another flag.
- **No repository content is transmitted.** The only value sent is the host, owner, and repository name already parsed from the origin remote by spec 013. No file names, no file contents, no commit messages, no author identities, no local paths.
- Requests carry a static user agent naming the tool and version, and no other identifying header.
- No telemetry, under any flag. The tool does not report its own usage anywhere.
- Requests use a bounded timeout and are not retried beyond a documented limit. A failure degrades the field to `unavailable` with a reason; it never fails the run.
- Unauthenticated by default. A forge token is read from the environment only when the user sets one, is never written anywhere, and never appears in output or diagnostics.
- Responses are cached outside the scanned repository, under the platform cache directory, with a documented TTL. Cache writes obey invariant I5 of [000](000-safety-invariants.md): nothing is written inside the repository under inspection.

## Behavior

```text
repo-radar scan [PATH] --network
repo-radar brief [PATH] --network
```

When enabled and an origin remote resolves to a supported forge, report:

- Description and homepage as the forge states them
- Topics or tags
- Stars, forks, watchers, and open issue and pull request counts
- Primary language as the forge classifies it
- Licence as the forge reports it, alongside the locally detected licence from spec 013, with disagreement shown rather than reconciled
- Default branch, compared against the local checkout
- Archived, disabled, template, and fork flags
- Parent repository when the project is a fork, with ahead and behind counts against it
- Latest release: tag, date, and whether the local `HEAD` predates it
- Last push date, compared against the local last-commit date, giving a staleness figure for the clone itself

Every field records the forge, the endpoint that produced it, and the retrieval time. Cached values are labelled as cached with their age.

### Supported Forges

| Forge | Status |
| --- | --- |
| GitHub | Implemented in this phase |
| GitLab | Registered, unimplemented |
| Codeberg / Gitea | Registered, unimplemented |

Detection is by host, from a versioned table. An unrecognized host reports `unsupported forge` with the host name, not an error.

## Acceptance Criteria

1. Without `--network`, no socket is opened, asserted by a test that runs the full command with network access denied.
2. A request body and query string are asserted to contain no path, file name, or commit message from the local repository.
3. A remote URL containing embedded credentials never reaches a request, a cache entry, or any output.
4. A timeout, a non-success status, and a rate-limit response each degrade to `unavailable` with a distinct stated reason, and the command still exits `0`.
5. An unrecognized host reports `unsupported forge` and performs no request.
6. A repository with no origin remote reports `not evaluated` and performs no request.
7. Cache entries are written outside the scanned repository; the spec 000 immutability harness passes with `--network` active.
8. A cached value is labelled as cached and states its age; a stale entry beyond the TTL is refetched.
9. Locally detected licence and forge-reported licence are both shown when they disagree, with neither presented as authoritative.
10. A token supplied by environment variable never appears in output, diagnostics, or the cache.
11. Forge-reported values are labelled as third-party claims, distinct from locally derived facts.
12. Tests use a local stub server and never contact a real forge.

## Constraints

- Forge responses are untrusted input. Descriptions, topics, and release names are escaped on output on the same terms as repository content.
- The forge is a source of claims, not of truth. Where a forge-reported value contradicts a locally derived one, both are reported.
- No write operation against any forge exists, under any flag. This is an instrument.
