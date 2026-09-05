# Feature Specification: Parallel Scanning

Status: Planned
Priority: P0
Depends on: `001-scan-engine`

## Goal

Make large repository scans fast by traversing directories in parallel, without changing any observable result.

## Behavior

- Directory traversal fans out across a worker pool while file metadata is collected concurrently.
- The public `scan` signature and `ScanReport` contents remain unchanged.
- Results stay deterministic: extension counts, largest-file ordering, and warning ordering must not depend on thread scheduling.
- The worker count is configurable through `ScanConfig` and defaults to the available parallelism of the host.
- A worker count of `1` selects the sequential path so the previous implementation stays comparable.
- Warnings are collected from all workers and sorted by path before the report is returned.

## Acceptance Criteria

1. A parallel scan and a sequential scan of the same fixture produce byte-identical JSON output.
2. Repeated parallel scans of the same fixture produce equal reports.
3. `ScanConfig` exposes worker count and the default is derived from `std::thread::available_parallelism`.
4. The benchmark reports sequential and parallel timings for the same fixture in one run.
5. The parallel scan is not slower than sequential on a fixture of at least 5,000 files.
6. No shared mutable state is guarded by a lock held across filesystem calls.
7. Clippy passes with warnings denied, including on the new dependency's feature set.

## Constraints

- Any added dependency must be a well-known parallelism crate such as `rayon`; hand-rolled thread pools are out of scope.
- Recursion depth must be bounded so a deep tree cannot overflow the stack; convert to an explicit work queue if needed.
- This phase adds no new report fields. Only performance changes.
