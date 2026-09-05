# Feature Specification: Dependency Intelligence

Status: Planned
Priority: P1
Depends on: `010-dependency-graph`, `014-project-profile`

## Goal

Turn a list of third-party dependencies into a judgement: what is here, how old it is, what it costs legally, and what could replace it.

## Behavior

```text
repo-radar deps [PATH] [--online] [--format text|json] [--fail-on LEVEL]
```

### Inventory (offline, always available)

For each dependency across supported ecosystems (Cargo, npm, PyPI, Go):

- Name, declared version requirement, and resolved locked version
- Kind: direct, development, build, optional, or transitive
- The manifest and lockfile entries that are the evidence
- Which subsystems consume it, from spec 016
- Usage weight: how many files import it
- Declared license, resolved to an SPDX identifier
- Duplicate detection: the same package present at multiple incompatible versions
- Unused declarations: a manifest dependency that no source file imports
- Missing declarations: an import with no matching manifest entry

### Licensing

- Resolve each dependency license to SPDX from a vendored SPDX identifier list
- Classify each as permissive, weak copyleft, strong copyleft, network copyleft, proprietary, or unknown
- Flag conflicts against the project's own declared license, with the rule that produced each flag stated in the output
- Report the strictest obligation present in the tree
- Report dependencies with no resolvable license as `unknown`, which is a finding, not a pass

### Freshness and alternatives

- Offline: compare declared versus locked versions, report packages pinned to a major version behind others in the same tree, and flag packages with no lockfile entry
- With `--online`: query the ecosystem registry for the latest published version, compute how many major, minor, and patch releases behind the locked version is, and report last-publish date as a maintenance signal
- Advisory matching against a local RustSec or OSV database when one is present on disk
- Alternatives come from a vendored, versioned catalogue mapping a package to its category and to peer packages, each with a one-line note on when the peer is the better fit

## Acceptance Criteria

1. No network request is made unless `--online` is passed. The default run is verifiably offline.
2. With `--online`, every network call is to a documented registry host, has a timeout, and a failure degrades to the offline result with a warning rather than failing the command.
3. `--online` never transmits repository content: only package names already published publicly.
4. Licence conflict findings state the rule and both licenses involved.
5. An unrecognized license string yields `unknown` and never a guessed SPDX identifier.
6. Duplicate, unused, and missing dependency detection each have a dedicated fixture.
7. `--fail-on LEVEL` exits non-zero when a finding at or above that severity exists, making the command usable in CI.
8. The alternatives catalogue is data, is versioned, records its last review date, and its suggestions are labelled as opinion rather than fact.
9. A repository with no dependencies produces a successful empty report.

## Constraints

- Repo Radar never installs, updates, resolves, builds, or modifies a manifest or lockfile.
- License classification is informational and must carry a plain statement that it is not legal advice.
- The alternatives catalogue must stay small and defensible. A stale or unreviewed recommendation is worse than no recommendation.
