# Design

## Domain Model

`run_state` remains the authority for the single active run. A live run gains
durable ownership (`owner_pid`, `agent_pid`, process-start identity), liveness
(`heartbeat_at`), control (`cancel_requested`), and presentation
(`current_stage`, `terminal_reason`) fields.

The canonical stages are `start`, `agent`, `validation`, `pr`, `review`,
`sync`, and `done`. A terminal failure retains its last stage.

## Application Flow

Preparing a run records stage Start. Spawning the adapter records Running,
process ownership, heartbeat, and Agent. Both adapter loops refresh heartbeat,
check cancellation, and emit normalized events. Successful execution advances
to Validation, then PR or Review. Merge/local acceptance advances to Sync, and
successful changeset application advances to Done.

Web startup reconciles active rows owned by a previous controller. It verifies
process identity before best-effort process-group termination, records
Interrupted, and releases the active lock. It never reattaches to a child.

## Interface Contract

`GET /api/runs/<run-id>/events?after=<sequence>` returns the retained normalized
events after the cursor plus `last_sequence` and `reset_required`. Omitting the
cursor preserves snapshot behavior and legacy Codex fallback.

`POST /api/runs/<run-id>/cancel` atomically requests cancellation for the
current active run. Repeated active requests are idempotent; terminal runs
return conflict and unknown runs return not found.

## Data Model

Add nullable/defaulted columns to `run_state`: owner PID, agent PID,
agent-process start identity, heartbeat timestamp, current stage, cancellation
flag, and terminal reason. Existing databases migrate additively.

Add `RUN_EVENTS.jsonl` as the normalized event artifact. Raw
`APP_SERVER_EVENTS.jsonl` and `AGENT_OUTPUT.log` remain adapter evidence.

## UI / Platform Impact

The task detail appends cursor-based event responses while an active run is
open. It presents a confirmed destructive Cancel action. The lifecycle uses
the backend stage model without changing board buckets. Unix process groups and
the existing non-Unix direct-child fallback remain supported and tested.

## Observability

Normalized events have sequence, timestamp, agent, kind, stage, and message.
Retention preserves recent and terminal events and reports when a client cursor
predates retained history. Runtime state exposes heartbeat and terminal reason
for diagnosis.

## Alternatives Considered

1. Adapter-specific endpoints were rejected because they preserve incompatible
   monitoring behavior.
2. A supervisor daemon with push events was rejected as disproportionate for a
   local single-run controller.

