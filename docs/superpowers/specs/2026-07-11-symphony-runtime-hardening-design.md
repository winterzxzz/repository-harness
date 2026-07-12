# Symphony Runtime Hardening Design

## Status

Approved direction based on the 2026-07-11 OpenAI Symphony comparison and
follow-up review. This design hardens the existing Harness-local, single-agent
runner; it does not attempt full OpenAI Symphony conformance.

## Goal

Make unattended local Symphony runs bounded, recoverable, and truthful: agent
processes must stop when required, crashes must not strand work, failed proof
must not become a completed PR, and runtime artifacts must have explicit size
and retention limits.

## Scope

Implement now:

1. Enforce agent wall-clock deadlines for Codex, OpenCode, and custom adapters.
2. Stream stdout and stderr without unbounded memory buffering or pipe deadlock.
3. Terminate the complete agent process tree on timeout, cancellation, or error.
4. Recover orphan active runs and queue rows after a Symphony process crash.
5. Add capped exponential retry backoff for unattended runs.
6. Roll back worktrees, branches, run directories, and partial state when
   preparation fails.
7. Reject `completed` results when validation contains `fail` or `unavailable`.
8. Pause unattended dispatch after an upstream refresh failure unless stale-base
   execution is explicitly enabled.
9. Cap/redact runtime logs and expose accurate runtime/termination information.
10. Verify Web UI identity through HTTP health data instead of TCP reachability.

Defer:

- External issue trackers such as Linear.
- Multiple concurrent agents.
- Repository `WORKFLOW.md` and dynamic workflow hooks.
- Distributed workers, remote Web UI, authentication, and auto-merge.

## Runtime Execution

All adapters use one supervised child-process abstraction. The supervisor owns
the start time, deadline, process-tree identity, stdout/stderr drains, bounded
log writers, and final termination reason. Adapter-specific code only prepares
arguments and interprets protocol output.

On Unix, the child starts in a new process group. Termination first requests a
graceful stop, waits for a short configured grace period, then kills the process
group. Windows uses a Job Object or the closest supported process-tree primitive.
The immediate implementation may keep platform-specific code behind small
helpers, but it must never silently fall back to killing only the parent.

Codex stdout remains a JSON-RPC stream. Stderr is drained concurrently and a
capped tail is retained for diagnostics. Custom and OpenCode output streams to
bounded artifacts instead of `Command::output()`.

## Timeouts And Outcomes

`agent.timeout_minutes` becomes a real wall-clock deadline. Zero does not mean
uncapped; configuration validation rejects zero. A timeout records a distinct
termination reason and produces an interrupted/failed run rather than relying
on substring matching in the Web UI.

Idle reconciliation remains useful for Codex protocol stalls, but it is
independent from the wall-clock deadline. The first condition reached wins and
is recorded in run state.

## Crash Recovery And Queue Leases

Running queue entries and active run rows gain ownership metadata:

- owner process ID
- owner process start identity or generated owner token
- heartbeat timestamp
- lease expiry timestamp

Before polling, auto mode performs reconciliation in one transaction. A running
row whose lease is expired and whose owner is no longer valid becomes
`interrupted`. Its queue entry is requeued when attempts remain, otherwise it is
terminally failed. Live leases are never stolen.

The worker refreshes its heartbeat while an agent is active. Queue claiming is
atomic so two local Symphony processes cannot claim the same story.

## Retry Backoff

Queue rows gain `next_attempt_at`. Failed attempts use capped exponential
backoff derived from the attempt number. Selection only returns queued rows
whose due time has passed. Tests use an injected clock so no test sleeps.

Default policy:

- initial retry delay: 10 seconds
- multiplier: 2
- maximum delay: 5 minutes
- existing `max_attempts` remains the terminal bound

## Transactional Preparation

Preparation uses a rollback guard that records each created resource. On error,
cleanup happens in reverse order: partial artifacts, run directory, worktree,
then branch. Successful state insertion commits the guard. Replacement and
ordinary preparation share this path to prevent behavioral drift.

## Validation And PR Gating

Result schema validation and success policy are separate:

- `completed` requires every declared validation command to report `pass`.
- `fail` forces a non-completed outcome.
- `unavailable` or a top-level unavailable reason may be reviewable, but cannot
  create a ready PR; it may create a draft only when policy explicitly permits.
- PR planning re-reads and validates `RESULT.json`; it does not trust only the
  stored run status.

This preserves honest evidence while allowing blocked or partial work to remain
inspectable.

## Upstream Freshness

Auto mode records the current base commit for every run. If upstream refresh
fails, dispatch pauses and reports the reason. An explicit
`auto.allow_stale_base: true` setting permits offline operation and records both
the base SHA and refresh failure in the run contract/result surface.

## Bounded Artifacts

Agent stdout, stderr, and app-server events use configurable byte limits. Once a
limit is reached, the writer records a truncation marker and stops persisting
additional content while continuing to drain the pipe. Sensitive headers and
known secret environment values are redacted before persistence.

Run records capture elapsed runtime, termination reason, and available Codex
token usage. Token budgets are enforced only when reliable cumulative usage is
available; wall-clock and byte limits remain mandatory fallbacks.

## Web Health Identity

`/health` returns a small JSON document containing service name, version, and a
stable hash of the repository root. `ensure_web_server` performs an HTTP health
request and reuses the listener only when identity matches. A foreign service or
a Symphony server for another repository produces a warning and does not count
as the expected controller.

The server remains loopback-only by default. Authentication is outside this
local-only design.

## Testing Strategy

Use test-first development for each behavior:

- fake agents that exceed deadlines, spawn descendants, flood stderr, and emit
  oversized output
- restart tests with orphan queue/run leases
- deterministic retry scheduling with an injected clock
- preparation failures injected after each created resource
- completed results containing failed or unavailable validation
- upstream refresh failure with and without stale-base opt-in
- HTTP health checks against a foreign listener, wrong repository, and matching
  Symphony server

Run focused crate tests after each red-green cycle, then the complete
`cargo test -p harness-symphony`, formatting, clippy, and installer/kit validators
affected by configuration or documentation changes.

## Compatibility

Existing successful local runs remain compatible. State schema changes use a
forward migration and preserve historical runs. New defaults are deliberately
fail-closed for timeout, stale bases, and completed validation. Documentation
will label this implementation as the Harness-local profile rather than a fully
conformant OpenAI Symphony service.
