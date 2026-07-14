# Symphony Agent Runtime Observability And Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Codex and OpenCode runs observable, cancellable, correctly staged, and safely recoverable without weakening Symphony isolation or validation.

**Architecture:** Add durable runtime-control fields to `run_state`, a focused normalized-event module, and adapter hooks for ownership, heartbeat, cancellation, and stages. Keep polling but add sequence cursors; reconcile stale Web-owned runs at startup and derive lifecycle UI from durable stages.

**Tech Stack:** Rust, rusqlite, serde/serde_json, local process groups, React, TypeScript, Playwright, Harness CLI.

---

## File Map

- Create `crates/harness-symphony/src/run_events.rs`: normalized event envelope,
  bounded artifact writer, cursor reader, and legacy conversion boundary.
- Modify `crates/harness-symphony/src/main.rs`: register the event module.
- Modify `crates/harness-symphony/src/state.rs`: additive runtime fields,
  process identity, heartbeat/cancel/stage transitions, and stale-run queries.
- Modify `crates/harness-symphony/src/agent.rs`: uncapped Codex loop and shared
  runtime control for Codex/OpenCode/custom child processes.
- Modify `crates/harness-symphony/src/run.rs`: stage transitions around result
  validation and terminal ownership cleanup.
- Modify `crates/harness-symphony/src/web.rs`: startup reconciliation, event
  cursor and cancel endpoints, PR/sync stages, and lifecycle derivation.
- Modify `crates/harness-symphony/web-ui/src/features/symphony/types.ts`:
  normalized event and cursor/cancel response types.
- Modify `crates/harness-symphony/web-ui/src/features/symphony/api.ts`: cursor
  event fetch and cancel client.
- Modify `crates/harness-symphony/web-ui/src/features/symphony/detail.tsx`:
  incremental event append and confirmed Cancel action.
- Modify `crates/harness-symphony/web-ui/src/main.tsx`: cancel orchestration and
  board refresh.
- Modify `crates/harness-symphony/web-ui/src/run-log.ts`: format normalized
  events while preserving legacy Codex formatting.
- Modify `crates/harness-symphony/web-ui/tests/board.spec.ts`: live OpenCode,
  cursor polling, cancel, interruption, and lifecycle regression coverage.
- Modify `docs/product/symphony-web-ui-controller.md`, `docs/SYMPHONY_SCOPE.md`,
  and affected story packets: align runtime and evidence contracts.

### Task 1: Add Durable Run Runtime Control

**Files:**
- Modify: `crates/harness-symphony/src/state.rs`
- Test: `crates/harness-symphony/src/state.rs`

- [ ] **Step 1: Write failing migration and transition tests**

Add tests that initialize a pre-change `run_state`, call `init`, and assert the
new columns read through `RunRecord`. Add a transition test using the intended
API:

```rust
store.begin_execution("run_1", 4100, 4200, "agent-start-identity", 1_721_000_000)?;
let running = store.show_run("run_1")?;
assert_eq!(running.status, "running");
assert_eq!(running.current_stage, "agent");
assert_eq!(running.owner_pid, Some(4100));
assert_eq!(running.agent_pid, Some(4200));
assert!(!running.cancel_requested);

store.request_cancel("run_1")?;
assert!(store.cancellation_requested("run_1")?);

store.finish_execution("run_1", "cancelled", "operator cancelled run")?;
let cancelled = store.show_run("run_1")?;
assert_eq!(cancelled.status, "cancelled");
assert_eq!(cancelled.owner_pid, None);
assert_eq!(cancelled.agent_pid, None);
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
cargo test -p harness-symphony state::tests::runtime -- --nocapture
```

Expected: compilation failure because runtime fields and methods do not exist.

- [ ] **Step 3: Implement additive fields and atomic transitions**

Extend `RunRecord` with:

```rust
pub owner_pid: Option<u32>,
pub agent_pid: Option<u32>,
pub agent_start_identity: Option<String>,
pub heartbeat_at: Option<i64>,
pub current_stage: String,
pub cancel_requested: bool,
pub terminal_reason: Option<String>,
```

Use `ensure_column` for nullable/defaulted columns. Implement
`begin_execution`, `refresh_heartbeat`, `set_stage`, `request_cancel`,
`cancellation_requested`, and `finish_execution` with guarded SQL updates.
`finish_execution` must clear owner/agent/heartbeat/cancel fields but preserve
stage and terminal reason. Update every `SELECT` and `run_from_row` together.

- [ ] **Step 4: Add PID identity and stale-owner tests**

Test a live matching identity, absent process, reused PID identity, and unknown
identity. Expose a `stale_web_runs(current_owner_pid, probe)` helper that never
classifies the current controller as stale and never authorizes signalling a
mismatched identity.

- [ ] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p harness-symphony state::tests -- --nocapture
```

Expected: all state tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/harness-symphony/src/state.rs
git commit -m "feat: persist Symphony run runtime control"
```

### Task 2: Add Normalized Sequenced Run Events

**Files:**
- Create: `crates/harness-symphony/src/run_events.rs`
- Modify: `crates/harness-symphony/src/main.rs`
- Test: `crates/harness-symphony/src/run_events.rs`

- [ ] **Step 1: Write failing event writer and cursor tests**

Define the wished-for API in tests:

```rust
let writer = RunEventWriter::new(path.clone(), "run_1", "opencode")?;
writer.append("output", "agent", "first")?;
writer.append("lifecycle", "validation", "validating result")?;

let first = read_events_after(&path, None)?;
assert_eq!(first.events.len(), 2);
assert_eq!(first.last_sequence, 2);
assert!(!first.reset_required);

let delta = read_events_after(&path, Some(1))?;
assert_eq!(delta.events[0].sequence, 2);
```

Add a retention test that writes beyond the injected cap and proves the newest
terminal event remains present and an old cursor returns `reset_required`.

- [ ] **Step 2: Run and verify RED**

```bash
cargo test -p harness-symphony run_events::tests -- --nocapture
```

Expected: module/type resolution failure.

- [ ] **Step 3: Implement the focused module**

Use these public shapes:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunEvent {
    pub sequence: u64,
    pub timestamp: String,
    pub agent: String,
    pub kind: String,
    pub stage: String,
    pub message: String,
}

pub struct EventPage {
    pub events: Vec<RunEvent>,
    pub last_sequence: u64,
    pub reset_required: bool,
}
```

Serialize each append under one mutex and one `write_all` call. On compaction,
retain recent complete JSON lines, write a metadata marker containing the
highest dropped sequence, and always leave room for the new event. Do not
change the raw Codex or OpenCode artifact writers.

- [ ] **Step 4: Verify GREEN and formatting**

```bash
cargo test -p harness-symphony run_events::tests -- --nocapture
cargo fmt --check
```

- [ ] **Step 5: Commit**

```bash
git add crates/harness-symphony/src/main.rs crates/harness-symphony/src/run_events.rs
git commit -m "feat: add normalized Symphony run events"
```

### Task 3: Make Adapter Execution Controlled And Codex Uncapped

**Files:**
- Modify: `crates/harness-symphony/src/agent.rs`
- Modify: `crates/harness-symphony/src/run.rs`
- Test: `crates/harness-symphony/src/agent.rs`
- Test: `crates/harness-symphony/src/run.rs`

- [ ] **Step 1: Write a failing uncapped Codex regression**

Replace the deadline-oriented helper contract with an injected loop policy that
controls only heartbeat/stall timing. The fake app-server must remain
`inProgress` beyond an injected former wall-clock duration and then return
`completed`. Assert success instead of `AgentError::Timeout`.

- [ ] **Step 2: Run and verify RED**

```bash
cargo test -p harness-symphony agent::tests::codex_run_is_not_failed_by_elapsed_wall_clock -- --nocapture
```

Expected: failure from the current absolute deadline.

- [ ] **Step 3: Remove only the Codex absolute deadline**

Keep `agent_timeout(config)` in `run_streaming_command` for OpenCode/custom.
Remove the `deadline` comparison from the Codex loop. Keep the existing
idle-state reconciliation request and the second no-response stall failure.
Update `agent_adapter_status` so Codex reports uncapped while OpenCode reports
its configured timeout.

- [ ] **Step 4: Write failing adapter-control tests**

For Codex and OpenCode fake processes, assert:

- spawn records running ownership and Agent stage;
- heartbeat advances while the process is live;
- setting `cancel_requested` kills a spawned descendant;
- cancellation returns a distinct internal cancellation outcome;
- OpenCode stdout/stderr creates normalized events and retains
  `AGENT_OUTPUT.log`.

- [ ] **Step 5: Implement shared adapter runtime control**

Pass a small `AgentRuntime` built from state DB, run metadata, and
`RunEventWriter` into adapter loops. After spawn call `begin_execution`; on loop
wake refresh heartbeat at a bounded cadence and check cancellation. Map Codex
notifications/messages and OpenCode output chunks to normalized events. Ensure
every error path terminates `ProcessTreeGuard` before returning.

- [ ] **Step 6: Stage validation and clear terminal ownership**

In `execute_prepared_run`, set stage `validation` after agent success and before
`validate_finished_run`. Use `finish_execution` for failed, cancelled,
interrupted, and validated terminal outcomes so ownership cannot leak.

- [ ] **Step 7: Verify GREEN**

```bash
cargo test -p harness-symphony agent::tests -- --nocapture
cargo test -p harness-symphony run::tests -- --nocapture
```

- [ ] **Step 8: Commit**

```bash
git add crates/harness-symphony/src/agent.rs crates/harness-symphony/src/run.rs
git commit -m "fix: control agent runtime and uncap Codex"
```

### Task 4: Add Startup Reconciliation And Cancel API

**Files:**
- Modify: `crates/harness-symphony/src/web.rs`
- Test: `crates/harness-symphony/src/web.rs`

- [ ] **Step 1: Write failing startup reconciliation tests**

Seed a running row with a prior owner and absent child. Start the Web server
through an injected reconciliation helper and assert status `interrupted`,
terminal reason is actionable, and a new run can be added. Add a mismatched PID
identity test that proves no signal is sent.

- [ ] **Step 2: Run and verify RED**

```bash
cargo test -p harness-symphony web::tests::startup_reconciles_stale_web_run -- --nocapture
```

- [ ] **Step 3: Implement reconciliation before accepting requests**

Call `reconcile_web_runs` in `run_web_server` before `serve`. Use injectable
process probing/signalling in tests. Interrupt prior-owner runs; best-effort
terminate only matching process identities; retain artifacts and worktrees.

- [ ] **Step 4: Write failing cancel endpoint tests**

Cover accepted active cancellation, repeated idempotent request, unknown run
404, terminal run 409, and cancellation of a different non-active run 409.
Expected route: `POST /api/runs/run_1/cancel`.

- [ ] **Step 5: Implement the cancel route**

Add a serialized response containing `run_id`, `status: "cancelling"`, and
`cancel_requested: true`. Atomically guard `request_cancel` so only the current
active run can change. Add the path to the method-not-allowed route set.

- [ ] **Step 6: Verify GREEN and commit**

```bash
cargo test -p harness-symphony web::tests -- --nocapture
git add crates/harness-symphony/src/web.rs
git commit -m "feat: reconcile and cancel Web-started runs"
```

### Task 5: Add Cursor Event API With Legacy Fallback

**Files:**
- Modify: `crates/harness-symphony/src/web.rs`
- Test: `crates/harness-symphony/src/web.rs`

- [ ] **Step 1: Write failing API tests**

Write three tests: no cursor returns retained normalized snapshot; `after=2`
returns only sequence 3+; missing `RUN_EVENTS.jsonl` falls back to legacy
`APP_SERVER_EVENTS.jsonl` without failing old completed runs.

- [ ] **Step 2: Run and verify RED**

```bash
cargo test -p harness-symphony web::tests::events_request_supports_sequence_cursor -- --nocapture
```

- [ ] **Step 3: Implement parsing and response**

Return:

```json
{
  "run_id": "run_1",
  "events": [],
  "last_sequence": 12,
  "reset_required": false
}
```

Reject malformed/negative cursors with 400. Keep legacy events valid by
wrapping them only at the response boundary; do not rewrite old artifacts.

- [ ] **Step 4: Verify GREEN and commit**

```bash
cargo test -p harness-symphony web::tests::events -- --nocapture
git add crates/harness-symphony/src/web.rs
git commit -m "feat: expose cursor-based run events"
```

### Task 6: Drive Lifecycle From Durable Stages

**Files:**
- Modify: `crates/harness-symphony/src/run.rs`
- Modify: `crates/harness-symphony/src/web.rs`
- Test: `crates/harness-symphony/src/web.rs`

- [ ] **Step 1: Write failing stage derivation tests**

Assert current lifecycle steps for running Agent, Validation, PR creation,
Review, merged Sync, failed-at-stage, cancelled-at-stage, and synced Done. The
Done response must remain owned long enough to render all steps complete rather
than immediately returning `task_flow: null`.

- [ ] **Step 2: Run and verify RED**

```bash
cargo test -p harness-symphony web::tests::task_flow_uses_durable_stage -- --nocapture
```

- [ ] **Step 3: Record authoritative stage transitions**

Set `pr` before PR creation, `review` after PR/local-review readiness, `sync`
after merge/local acceptance, and `done` after successful sync. On failure,
retain the stage and existing recovery action.

- [ ] **Step 4: Refactor `derive_task_flow` minimally**

Select the active run first, then the newest Review/Needs Attention/Done owner.
Map `run.current_stage` to the seven canonical steps. Treat terminal completed
Done as all complete and cancelled/interrupted as failed at the retained step.
Do not change board bucket keys.

- [ ] **Step 5: Verify GREEN and commit**

```bash
cargo test -p harness-symphony web::tests::task_flow -- --nocapture
git add crates/harness-symphony/src/run.rs crates/harness-symphony/src/web.rs
git commit -m "feat: derive task lifecycle from runtime stages"
```

### Task 7: Add Incremental Web Monitoring And Cancel UI

**Files:**
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/types.ts`
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/api.ts`
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/detail.tsx`
- Modify: `crates/harness-symphony/web-ui/src/main.tsx`
- Modify: `crates/harness-symphony/web-ui/src/run-log.ts`
- Modify: `crates/harness-symphony/web-ui/tests/board.spec.ts`

- [ ] **Step 1: Write failing parser tests**

Define `NormalizedRunEvent`, add `last_sequence` and `reset_required` to
`EventsResponse`, and test strict parsing of sequence/kind/stage/message plus
legacy event acceptance.

- [ ] **Step 2: Write failing Playwright scenarios**

Cover OpenCode output appearing while active, a second poll including a concrete
cursor such as `?after=12`, reset replacing stale entries, Cancel confirmation
and POST, cancellation disabling the action, and lifecycle Validation/PR/Done.

- [ ] **Step 3: Run and verify RED**

```bash
npm --prefix crates/harness-symphony/web-ui run e2e -- --grep "runtime events|cancel run|durable lifecycle"
```

- [ ] **Step 4: Implement cursor polling**

Change `fetchEvents(runId, after, options)` to add the query parameter. In task
detail, replace events on initial/reset responses and append otherwise, dedupe
by sequence, retain the last cursor, and keep the current two-second cadence.
Format normalized events directly and fall back to the legacy formatter.

- [ ] **Step 5: Implement Cancel UI**

Add `postCancelRun`. Thread `onCancel` and pending run ID through `main.tsx` and
task detail. Show `Cancel run` only for `item.active_run`, require
`window.confirm`, disable while pending, show success/error toast, and reload
the board after the request.

- [ ] **Step 6: Verify GREEN**

```bash
npm --prefix crates/harness-symphony/web-ui run build
npm --prefix crates/harness-symphony/web-ui run e2e -- --grep "runtime events|cancel run|durable lifecycle"
```

- [ ] **Step 7: Commit**

```bash
git add crates/harness-symphony/web-ui/src crates/harness-symphony/web-ui/tests/board.spec.ts
git commit -m "feat: monitor and cancel active agent runs"
```

### Task 8: Align Contracts And Complete Verification

**Files:**
- Modify: `docs/product/symphony-web-ui-controller.md`
- Modify: `docs/SYMPHONY_SCOPE.md`
- Modify: `docs/stories/epics/E08-symphony-web-ui-controller/US-065-unlimited-codex-app-server-runtime.md`
- Modify: `docs/stories/epics/E08-symphony-web-ui-controller/US-078-opencode-agent-selection.md`
- Modify: `docs/stories/US-090-symphony-active-task-flow.md`
- Modify: `docs/stories/epics/E08-symphony-web-ui-controller/US-093-agent-runtime-observability-recovery/overview.md`
- Modify: `docs/stories/epics/E08-symphony-web-ui-controller/US-093-agent-runtime-observability-recovery/validation.md`

- [ ] **Step 1: Update documentation**

Document normalized events, cursor polling, adapter timeout distinction,
cancel semantics, startup interruption, lifecycle stages, raw artifact
compatibility, and exact recovery behavior. Mark related story prose consistent
without claiming proof that has not run.

- [ ] **Step 2: Run focused and full verification**

```bash
cargo fmt --check
cargo test -p harness-symphony
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix crates/harness-symphony/web-ui run build
npm --prefix crates/harness-symphony/web-ui run e2e
git diff --check
```

Expected: every command exits zero with no warnings promoted by clippy and all
Playwright scenarios pass.

- [ ] **Step 3: Run available platform smoke**

```bash
scripts/bin/harness-cli query tools --capability platform-smoke --status present
```

Run the registered `web-ui-desktop-smoke` command if present. Record a concrete
unavailable reason instead of fabricating platform proof if it cannot run.

- [ ] **Step 4: Update durable story evidence**

```bash
scripts/bin/harness-cli story update --id US-093 --status implemented --unit 1 --integration 1 --e2e 1 --platform 1 --evidence "cargo fmt --check, cargo test -p harness-symphony, cargo test --workspace, cargo clippy --workspace -- -D warnings, Web UI build, Playwright E2E, desktop smoke, and git diff --check passed."
scripts/bin/harness-cli story verify US-093
```

Use `--platform 0` with the exact unavailable reason if platform smoke is not
available.

- [ ] **Step 5: Record final trace and commit**

```bash
scripts/bin/harness-cli trace --intake 31 --story US-093 --agent symphony --outcome completed --summary "Implemented observable, cancellable, recoverable Codex and OpenCode runtimes" --actions "Added durable runtime control, normalized cursor events, cancellation, startup reconciliation, and lifecycle stages" --changed "crates/harness-symphony/src/main.rs,crates/harness-symphony/src/state.rs,crates/harness-symphony/src/run_events.rs,crates/harness-symphony/src/agent.rs,crates/harness-symphony/src/run.rs,crates/harness-symphony/src/web.rs,crates/harness-symphony/web-ui/src/features/symphony/types.ts,crates/harness-symphony/web-ui/src/features/symphony/api.ts,crates/harness-symphony/web-ui/src/features/symphony/detail.tsx,crates/harness-symphony/web-ui/src/main.tsx,crates/harness-symphony/web-ui/src/run-log.ts,crates/harness-symphony/web-ui/tests/board.spec.ts,docs/product/symphony-web-ui-controller.md,docs/SYMPHONY_SCOPE.md" --decisions "Applied decision 0009" --notes "Rust fmt, focused/workspace tests, clippy, Web build, browser E2E, platform smoke, story verify, and diff check passed."
git add docs crates
git commit -m "docs: record Symphony runtime recovery evidence"
```
