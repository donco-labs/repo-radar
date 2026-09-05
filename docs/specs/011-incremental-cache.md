# Feature Specification: Incremental Cache

Status: Planned
Priority: P1
Depends on: `001-scan-engine`, `002-structured-output`, `007-parallel-scanning`

## Goal

Make a repeat scan of an unchanged repository near-instant, and make a scan after a small change proportional to what changed.

## Behavior

Persist a cache keyed by absolute repository root:

- Stored per file: relative path, byte size, modification time, content hash, and any derived analysis results
- Cache location follows the platform cache directory and is overridable with `--cache-dir`
- On scan, a file is considered unchanged when size and modification time match; the content hash resolves ambiguity when timestamps are unreliable
- The report states cache hits, misses, and evictions
- `--no-cache` performs a full scan and does not write, and `--rebuild-cache` performs a full scan and overwrites

## Acceptance Criteria

1. A cached scan of an unchanged fixture produces a report equal to an uncached scan of the same fixture.
2. Modifying one file causes exactly one file to be reanalyzed, verified through the reported cache statistics.
3. Deleting a file removes it from the next report and from the cache.
4. A corrupt, truncated, or unreadable cache file is discarded with a warning and the scan still succeeds.
5. A cache written by an older cache schema version is discarded rather than misread.
6. A change to ignore configuration invalidates the cache for the affected root.
7. Two concurrent scans of the same root do not corrupt the cache.
8. A benchmark records cold-scan versus warm-scan time on a fixture of at least 5,000 files.

## Constraints

- The cache is an optimization and is never authoritative. Any inconsistency must resolve toward rescanning.
- The cache lives outside the scanned repository and never writes into the user's working tree.
- The cache format is versioned and its version is independent of the JSON output schema version.
