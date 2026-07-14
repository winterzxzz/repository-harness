# US-093 Review Hardening Design

## Context

PR #23 implements durable Symphony runtime ownership, normalized run events,
cancellation, and controller-startup recovery. Review found three valid problem
clusters: event persistence performs unnecessary full-file work and exposes
non-atomic rewrites; process recovery has an unsafe PID-zero edge case and
polls process state too aggressively; and state-finalization failures can hide
the execution or validation error that triggered them.

One review thread is not applicable to the current head: the Codex command is
already passed through `configure_process_group` before spawn. That thread will
be answered with the current code location rather than changed.

## Outcome

The event stream remains bounded and cursor-compatible without quadratic
append behavior, timestamps conform to the frontend RFC 3339 contract, startup
recovery cannot signal the controller's own process group through PID zero,
and callers keep the primary agent or validation failure even if terminal state
persistence also fails.

## Event Persistence

`RunEventWriter::new` creates the event directory once. Appends emit UTC RFC
3339 timestamps and only trigger compaction after the configured event limit is
exceeded and a bounded compaction interval is reached. Production uses an
interval of at most 100 events; small test limits use a proportionally small
interval so cursor and retention behavior stays testable.

Compaction writes the replacement through a temporary file in the destination
directory and atomically persists it over `RUN_EVENTS.jsonl`. Keeping the temp
file on the same filesystem preserves rename atomicity for concurrent readers.
The writer mutex continues to serialize appends from cloned writers.

## Process Recovery

Process-group conversion rejects zero and values that cannot fit the platform
PID representation before calling `kill`. Recovery polls at a coarser bounded
interval and obtains identity plus zombie state from one process probe per
attempt instead of spawning a second `ps` command. Unknown probe results remain
fail-closed: the active-run lock is not released unless the recorded process is
confirmed gone, replaced, or non-executing.

The existing graceful-then-forced termination sequence remains unchanged:
`SIGTERM`, bounded verification, `SIGKILL`, bounded verification. Windows keeps
its `taskkill /T /F` path and uses the same identity verification contract.

## Primary Error Preservation

When agent execution or result validation already failed, a subsequent
`finish_execution` failure is reported as a warning but does not replace the
primary error returned to the caller. Startup reconciliation remains able to
repair the stale running record. Successful execution still treats state
transition failures as fatal because no primary failure exists to preserve.

## Validation Strategy

Regression tests are written and observed failing before each production
change. They cover:

- RFC 3339 event timestamps;
- directory creation during writer initialization;
- batched compaction decisions and retained cursor behavior;
- temporary-file replacement behavior;
- rejection of PID zero without invoking a signal;
- one process probe per bounded wait attempt;
- preservation of agent and validation errors when state finalization fails.

Release gates are `cargo test --workspace`, workspace clippy with warnings as
errors, formatting, install-payload validation, and `git diff --check`. After
the branch is pushed, each review thread is answered and resolved, PR metadata
is re-read, and merge proceeds only when GitHub reports a mergeable clean head
with no unresolved actionable threads.
