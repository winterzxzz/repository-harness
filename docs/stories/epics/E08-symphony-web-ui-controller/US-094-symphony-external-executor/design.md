# Design

## Domain Model

Add `execution_mode=managed|external` to run state. Managed runs keep PID
ownership; external runs use a heartbeat lease with a 120-second default TTL.
Prepared and running remain the only active statuses. Stale external runs
release the lock and appear as Needs Attention.

The existing `agent` value identifies the executor. Result completion preserves
the canonical result outcome rather than adding a `done` run status.

## Application Flow

The main agent prepares the run, starts the external lease, spawns the subagent,
refreshes the lease at least once every 30 seconds, forwards changed milestones
as normalized events, and completes the run after the subagent returns artifacts.

Subagents operate only in the prepared worktree. Active-dependent state access
reconciles expired leases transactionally; the Web server also sweeps on a
bounded timer.

## Interface Contract

```text
harness-symphony runs start <run_id> --executor <name>
harness-symphony runs heartbeat <run_id> [--step <text>]
harness-symphony runs complete <run_id>
```

Commands operate on the source repository, using cwd or global `--repo-root`.
Start accepts only the active prepared run. Heartbeat accepts only running
external runs. Complete accepts running or stale external runs.

## Data Model

- Add non-null `execution_mode` with a `managed` default for migrated rows.
- Add `runs.external_heartbeat_ttl_seconds` with a 120-second default.
- Store a canonical logical digest of the copied Harness DB for completion
  validation.
- Reuse `agent`, `heartbeat_at`, `current_stage`, `terminal_reason`, and the
  normalized event artifact.
- Do not add a duplicate executor field or a second status store.

## UI / Platform Impact

Run card and detail show the existing agent value as an Executor badge.
Heartbeat steps use normalized events. Stale external runs derive Needs
Attention without adding a board column. Behavior must remain consistent in
browser and Electron surfaces.

## Observability

Start, changed milestones, expiry, validation, and completion emit normalized
events. Ordinary heartbeat refreshes update durable liveness without flooding
the event stream.

## Alternatives Considered

1. Main-agent-owned external lease: selected.
2. Subagent writes root control state: rejected for isolation.
3. Artifact-polling adapter: rejected for implicit coordination.
4. Supervisor daemon: rejected as disproportionate.
