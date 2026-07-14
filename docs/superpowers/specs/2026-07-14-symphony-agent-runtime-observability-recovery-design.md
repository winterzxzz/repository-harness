# Symphony Agent Runtime Observability And Recovery Design

## Intent

Make Web-started Codex and OpenCode runs observable and recoverable for their
entire lifecycle. Codex must not fail because an arbitrary wall-clock deadline
elapsed, both adapters must expose useful live progress, and a controller crash
must not leave an active-run lock that permanently blocks future work.

The change also adds an explicit cancel action. Cancellation is a guarded run
lifecycle transition, not a database edit or worktree deletion.

## Current Problems

The current implementation has four related gaps:

1. The Codex adapter still creates a deadline from `agent_timeout_minutes`, even
   though product documentation and `config show` describe Codex runtime as
   uncapped.
2. Codex writes `APP_SERVER_EVENTS.jsonl`, while OpenCode writes only
   `AGENT_OUTPUT.log`. The Web event endpoint reads only the Codex artifact, so
   an active OpenCode run appears silent.
3. Web-started execution runs in an in-process detached thread without durable
   process ownership or heartbeat data. A controller crash can leave a
   `prepared` or `running` record that continues to hold the single-active-run
   lock.
4. The lifecycle UI is derived from broad board states. Successful work stays
   on Agent and skips visible Validation and Pull Request activity.

## Approved Scope

### In scope

- Remove the fixed wall-clock deadline from Codex app-server execution.
- Preserve the documented Codex protocol-stall guard and terminal failure
  handling.
- Keep the existing configurable timeout for OpenCode and custom commands.
- Persist durable owner, process, heartbeat, cancellation, and current-stage
  state for Web-started runs.
- Reconcile orphaned Web runs when the controller starts.
- Add a guarded API and Web UI action to cancel the active run.
- Emit a normalized, sequenced event stream for Codex and OpenCode.
- Let the event API return only events after a client-provided sequence.
- Drive the seven-step lifecycle from durable current-stage state.
- Preserve existing raw artifacts and backward-compatible review behavior.

### Out of scope

- Multiple simultaneous runs.
- Remote execution or a separate supervisor daemon.
- WebSocket or Server-Sent Events transport.
- Redesigning the board, task detail, review, PR, or sync surfaces.
- Automatically resuming an agent process after the controller restarts.
- Changing worktree isolation, validation requirements, or PR policy.

## Approaches Considered

### Normalized event pipeline with durable run ownership

Selected. Both adapters feed one normalized event artifact and update one
durable runtime record. The existing polling architecture remains, but polling
uses sequence cursors. This provides consistent UI behavior without adding a
new daemon or network protocol.

### Minimal adapter-specific patch

Rejected. Exposing `AGENT_OUTPUT.log` separately and clearing stale records on
startup would be smaller, but it would preserve two incompatible monitoring
models and make lifecycle stage updates adapter-specific.

### External supervisor daemon with push events

Rejected for this slice. It would provide the strongest process ownership and
true push updates, but it introduces service lifecycle, IPC, packaging, and
cross-platform installation work that is disproportionate for a local
single-run controller.

## Architecture

### Durable runtime state

Extend `run_state` with nullable/defaulted fields so existing databases migrate
in place:

- `owner_pid`: PID of the Symphony controller that owns execution.
- `agent_pid`: PID of the direct Codex/OpenCode/custom child process.
- `heartbeat_at`: last Unix timestamp written by the live execution loop.
- `current_stage`: `start`, `agent`, `validation`, `pr`, `review`, `sync`, or
  `done`.
- `cancel_requested`: boolean integer, default false.
- `terminal_reason`: optional concise reason for cancelled or interrupted runs.

Preparation creates the run at stage `start`. After the child is spawned, the
adapter atomically records `running`, both PIDs, stage `agent`, and the initial
heartbeat. Terminal status updates clear live ownership fields but retain the
last stage and terminal reason for review.

SQLite remains the authority for the single-active-run lock. Concurrent start
protection continues to use the existing immediate transaction.

### Execution control

Introduce a small runtime-control abstraction shared by all adapters. It owns
only these responsibilities:

- register the spawned child PID;
- refresh the run heartbeat;
- check the durable cancellation flag;
- mark stage transitions;
- append normalized events.

The Codex receive loop already wakes every 250 ms and can check cancellation
and refresh heartbeat there. The streaming command loop used by OpenCode and
custom adapters already wakes every 10 ms; it should use a less frequent
heartbeat schedule while checking cancellation promptly.

Cancellation terminates the existing process group through
`ProcessTreeGuard`, waits for the process tree to exit, and records status
`cancelled`. It must not delete the worktree, run artifacts, or partial output.

### Uncapped Codex runtime

Codex execution has no absolute deadline. It terminates only when:

- Codex reports a completed, failed, or interrupted terminal turn;
- the app-server process exits;
- the operator requests cancellation;
- the documented idle reconciliation request receives no response within the
  protocol-stall window; or
- writing required runtime evidence fails.

`agent_timeout_minutes` continues to apply to OpenCode and custom commands.
Configuration and doctor output must describe this distinction accurately.

### Normalized events

Add `RUN_EVENTS.jsonl` beside the existing run artifacts. Each line uses a
stable envelope:

```json
{
  "sequence": 12,
  "timestamp": "2026-07-14T10:30:00Z",
  "agent": "opencode",
  "kind": "output",
  "stage": "agent",
  "message": "Running cargo test -p harness-symphony"
}
```

Required kinds are `lifecycle`, `message`, `output`, `warning`, and `error`.
The event writer assigns monotonically increasing sequence numbers per run and
serializes writes so stdout, stderr, and lifecycle events cannot interleave
partial JSON lines.

Codex continues to retain raw JSON-RPC in `APP_SERVER_EVENTS.jsonl` for
debugging. Its meaningful lifecycle and message notifications are also mapped
to normalized events. OpenCode continues to retain combined raw output in
`AGENT_OUTPUT.log`; stdout and stderr chunks additionally produce normalized
output events. Existing completed runs that have no normalized artifact fall
back to the legacy Codex event reader.

Normalized event retention must preserve recent and terminal events rather
than freezing permanently at the first size cap. When compaction drops old
events, the artifact records the highest dropped sequence so clients can reset
cleanly.

### Event API and polling

`GET /api/runs/<run-id>/events` accepts an optional non-negative `after`
sequence. The response includes:

- `run_id`;
- `events` whose sequence is greater than `after`;
- `last_sequence`;
- `reset_required`, true when the requested cursor predates retained events.

Without `after`, the endpoint returns the retained snapshot for compatibility.
The active task detail loads an initial snapshot, then polls every two seconds
with the latest sequence and appends only new events. Board polling remains at
its existing cadence and continues to own board/lifecycle refresh.

### Startup reconciliation

On Web controller startup, inspect every run in `prepared` or `running` state:

- A `prepared` run with no child and no fresh owner becomes `interrupted` after
  startup reconciliation.
- A run owned by the current controller remains active.
- A run owned by a previous controller is not resumed because stdout/stderr
  pipes cannot be reattached safely.
- If a recorded agent PID/process identity is still live, terminate its process
  group best-effort before marking the run `interrupted`.
- If process identity cannot be verified, fail closed: do not signal an
  unrelated PID, mark the run `interrupted`, and record that manual process
  inspection may be required.

PID reuse protection should reuse the process-start identity pattern already
used by auto-queue lease recovery rather than trusting a numeric PID alone.

### Cancel API and UI

Add `POST /api/runs/<run-id>/cancel`.

The endpoint succeeds only when the requested run is the current
`prepared`/`running` run. It sets `cancel_requested` atomically and returns an
accepted response. Repeated requests are idempotent. Terminal runs return a
conflict with their current status; unknown runs return not found.

The active task detail shows a destructive `Cancel run` action with a browser
confirmation. While cancellation is pending, the control is disabled and the
UI continues polling until the run becomes `cancelled`. The failure/recovery
surface treats cancellation as intentional and offers a safe execution retry
without describing it as an agent crash.

### Lifecycle stages

Durable stage transitions are:

```text
prepare → start
child spawned → agent
agent terminal success → validation
validated completed result with PR enabled → pr
PR ready or local review required → review
PR merged or local review accepted → sync
changeset applied → done
```

A failure retains the stage at which it occurred. Cancellation during agent
execution retains `agent`; startup interruption retains the last durable
stage. The board API derives the lifecycle from `current_stage` and terminal
status, while existing board buckets remain unchanged.

## Error Handling

- A process spawn failure records `failed` at stage `agent` and clears ownership.
- A heartbeat write failure terminates the child rather than leaving an
  untracked process.
- A normalized event write failure terminates the child because observability
  is part of the run contract.
- Malformed legacy event lines remain skippable with a diagnostic.
- Cancel signalling failure records an actionable terminal reason and leaves
  artifacts intact.
- Startup reconciliation never signals a PID unless its recorded process-start
  identity matches.
- Validation, PR, review, and sync failures retain their existing recovery
  actions and gain the correct failed lifecycle stage.

## Compatibility And Migration

- Additive SQLite columns use safe defaults and require no destructive data
  migration.
- Existing run rows without ownership data remain readable.
- Existing Codex raw event artifacts remain visible in review evidence.
- The no-cursor event API behavior remains available for existing clients.
- OpenCode/custom timeout configuration remains unchanged.
- `single_active_run` remains enforced.

## Validation Strategy

### Unit

- Codex fake app-server remains alive past an injected former deadline and
  completes only on terminal status.
- Codex protocol-stall, process-exit, failed-turn, and cancellation paths
  terminate the process tree.
- OpenCode stdout and stderr create ordered normalized events while preserving
  `AGENT_OUTPUT.log`.
- Event retention preserves terminal events and reports cursor reset.
- State migrations, heartbeat updates, stage transitions, cancellation, and
  PID identity checks are covered.

### Integration

- Start API records durable ownership and stage transitions.
- Event API cursor requests return only newer events.
- Cancel API stops a fake descendant process and produces a cancelled run.
- Web startup reconciles a stale record and releases the active-run lock.
- Validation, PR, review, sync, and done stages are derived from authoritative
  run state.

### End-to-end

- A fake Codex run displays incremental normalized events and lifecycle stages.
- A fake OpenCode run displays live output rather than an empty monitor.
- Cancelling an active run updates the board and permits a subsequent run.
- A simulated controller restart surfaces interruption and permits retry.

### Release proof

- `cargo fmt --check`
- `cargo test -p harness-symphony`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `npm --prefix crates/harness-symphony/web-ui run build`
- `npm --prefix crates/harness-symphony/web-ui run e2e`
- `git diff --check`

## Rollout

Implement in vertical slices: durable runtime control first, uncapped Codex and
cancel second, normalized events third, lifecycle/UI integration fourth. Each
slice must retain a green workspace before the next begins. No existing active
run should be present while upgrading because startup reconciliation will
classify legacy active records without ownership metadata as interrupted.
