# Symphony Active Task Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an always-visible, compact seven-step lifecycle timeline above the Symphony board using normalized backend state.

**Architecture:** Extend the existing board API with a single normalized `task_flow` model derived from durable run, PR, review, and sync state. Render that model through a focused React component above the current status rail; reuse the board refresh loop and existing recovery contract instead of parsing log text or adding another state store.

**Tech Stack:** Rust, rusqlite, serde, React, TypeScript, Tailwind CSS, lucide-react, Playwright.

---

## File Structure

- Modify `crates/harness-symphony/src/state.rs`: expose run update ordering needed to select the current lifecycle owner deterministically.
- Modify `crates/harness-symphony/src/work.rs`: define lifecycle enums/model and derive the seven steps from authoritative Symphony state.
- Modify `crates/harness-symphony/src/web.rs`: serialize `task_flow` beside board items and cover API states.
- Modify `crates/harness-symphony/web-ui/src/features/symphony/types.ts`: define the normalized frontend contract.
- Modify `crates/harness-symphony/web-ui/src/features/symphony/api.ts`: validate `task_flow` at the response boundary.
- Create `crates/harness-symphony/web-ui/src/features/symphony/active-task-flow.tsx`: render the accessible horizontal timeline.
- Modify `crates/harness-symphony/web-ui/src/main.tsx`: preserve the last valid flow during silent refresh errors and place it above `SummaryStrip`.
- Modify `crates/harness-symphony/web-ui/tests/board.spec.ts`: prove idle, active, failed, review, done, reduced-motion, and narrow-screen behavior.

### Task 1: Derive A Deterministic Lifecycle Model

**Files:**
- Modify: `crates/harness-symphony/src/state.rs`
- Modify: `crates/harness-symphony/src/work.rs`
- Test: `crates/harness-symphony/src/work.rs`

- [ ] **Step 1: Write failing lifecycle derivation tests**

Add focused tests beside the existing board derivation tests. Build fixtures for idle, running, validation failure, PR review, merged PR awaiting sync, and done. Assert active runs outrank review runs and otherwise the most recently updated run wins.

```rust
#[test]
fn active_task_flow_prefers_active_run_and_maps_all_steps() {
    let items = vec![
        board_fixture("US-OLD", BoardState::Review, review_run("2026-07-13 10:00:00")),
        board_fixture("US-ACTIVE", BoardState::InProgress, running_run("2026-07-13 09:00:00")),
    ];
    let flow = derive_active_task_flow(&items).expect("flow");
    assert_eq!(flow.story_id.as_deref(), Some("US-ACTIVE"));
    assert_eq!(flow.current_step, Some(TaskFlowStepId::Agent));
    assert_eq!(flow.steps[0].state, TaskFlowStepState::Complete);
    assert_eq!(flow.steps[1].state, TaskFlowStepState::Current);
}

#[test]
fn active_task_flow_marks_the_failed_validation_step() {
    let flow = derive_active_task_flow(&[validation_failure_fixture()]).expect("flow");
    assert_eq!(flow.state, TaskFlowState::Failed);
    assert_eq!(flow.current_step, Some(TaskFlowStepId::Validation));
    assert_eq!(flow.steps[2].state, TaskFlowStepState::Failed);
}
```

- [ ] **Step 2: Run the tests and verify RED**

Run: `cargo test -p harness-symphony work::tests::active_task_flow -- --nocapture`

Expected: FAIL because `derive_active_task_flow`, lifecycle types, and timestamp data do not exist.

- [ ] **Step 3: Expose run update ordering and implement minimal lifecycle types**

Add `updated_at: String` to `RunRecord`, select it in every query that constructs the record, and use these exact domain types in `work.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFlowStepId { Start, Agent, Validation, Pr, Review, Sync, Done }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFlowStepState { Pending, Current, Complete, Failed }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFlowState { Active, Waiting, Failed, Done }

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskFlowStep { pub id: TaskFlowStepId, pub state: TaskFlowStepState }

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActiveTaskFlow {
    pub story_id: String,
    pub title: String,
    pub state: TaskFlowState,
    pub current_step: Option<TaskFlowStepId>,
    pub message: String,
    pub steps: Vec<TaskFlowStep>,
}
```

Derive the owner with priority `active_run.is_some()` first, then descending `run.updated_at`. Map `prepared|running` to Agent, result/validation failure to Validation, `pr_status=failed` to PR, created PR to Review, merged but unsynced to Sync, and synced/implemented to Done. Keep unknown states pending or failed; never mark them complete.

- [ ] **Step 4: Run focused and state tests**

Run: `cargo test -p harness-symphony work::tests::active_task_flow state::tests -- --nocapture`

Expected: PASS, including existing run record tests updated for `updated_at`.

- [ ] **Step 5: Commit the domain model**

```bash
git add crates/harness-symphony/src/state.rs crates/harness-symphony/src/work.rs
git commit -m "feat: derive Symphony active task lifecycle"
```

### Task 2: Publish The Normalized Board API Contract

**Files:**
- Modify: `crates/harness-symphony/src/web.rs`
- Test: `crates/harness-symphony/src/web.rs`

- [ ] **Step 1: Write failing board API tests**

Extend the existing `/api/board` tests to assert `task_flow: null` when idle and a serialized seven-step object for running, review, failed, sync, and done fixtures.

```rust
let body: serde_json::Value = serde_json::from_slice(response.body()).unwrap();
assert!(body["task_flow"].is_null());

let flow = &body["task_flow"];
assert_eq!(flow["story_id"], "US-FLOW");
assert_eq!(flow["current_step"], "agent");
assert_eq!(flow["steps"].as_array().unwrap().len(), 7);
```

- [ ] **Step 2: Run the API tests and verify RED**

Run: `cargo test -p harness-symphony web::tests::board_api -- --nocapture`

Expected: FAIL because `BoardResponse` has no `task_flow` field.

- [ ] **Step 3: Add `task_flow` to `BoardResponse`**

Construct item responses and the lifecycle from the same loaded board data. Preserve `recovery_action` by copying the selected owner's already-derived response action into a Web response wrapper:

```rust
#[derive(Debug, Serialize)]
struct TaskFlowResponse {
    #[serde(flatten)]
    flow: ActiveTaskFlow,
    recovery_action: Option<RecoveryAction>,
}

#[derive(Debug, Serialize)]
struct BoardResponse {
    items: Vec<BoardItemResponse>,
    task_flow: Option<TaskFlowResponse>,
}
```

Do not read event logs in the route. If validation failure classification needs evidence, reuse the existing result/failure-summary helpers that already inspect authoritative artifacts.

- [ ] **Step 4: Run the Web API suite**

Run: `cargo test -p harness-symphony web::tests -- --nocapture`

Expected: PASS with board response snapshots/field assertions updated.

- [ ] **Step 5: Commit the API contract**

```bash
git add crates/harness-symphony/src/web.rs
git commit -m "feat: expose task lifecycle from board API"
```

### Task 3: Validate The Frontend Contract

**Files:**
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/types.ts`
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/api.ts`
- Test: `crates/harness-symphony/web-ui/tests/board.spec.ts`

- [ ] **Step 1: Add failing response-boundary tests**

Add Playwright route fixtures for a valid idle `null` value and a complete model. Add malformed fixtures with an unknown step ID and fewer than seven ordered steps, then assert the existing board error surface reports a malformed response.

```ts
await route.fulfill({
  contentType: "application/json",
  body: JSON.stringify({
    items: [],
    task_flow: { story_id: "US-X", title: "X", state: "active", current_step: "mystery", message: "", steps: [], recovery_action: null }
  })
});
await expect(page.getByRole("alert")).toContainText("task_flow.current_step is invalid");
```

- [ ] **Step 2: Run parser tests and verify RED**

Run: `npm --prefix crates/harness-symphony/web-ui run e2e -- --grep "task flow response"`

Expected: FAIL because task-flow types and parsing do not exist.

- [ ] **Step 3: Add exact TypeScript types and strict parsers**

```ts
export type TaskFlowStepId = "start" | "agent" | "validation" | "pr" | "review" | "sync" | "done";
export type TaskFlowStepState = "pending" | "current" | "complete" | "failed";
export type TaskFlowState = "active" | "waiting" | "failed" | "done";
export type TaskFlow = {
  story_id: string;
  title: string;
  state: TaskFlowState;
  current_step: TaskFlowStepId | null;
  message: string;
  steps: Array<{ id: TaskFlowStepId; state: TaskFlowStepState }>;
  recovery_action: RecoveryAction | null;
};
export type BoardResponse = { items: BoardItem[]; task_flow: TaskFlow | null };
```

Validate enum members, exactly seven steps, canonical order, nullable current step, and existing recovery action shape. Do not silently coerce malformed flow data.

- [ ] **Step 4: Run response-boundary tests and TypeScript build**

Run: `npm --prefix crates/harness-symphony/web-ui run e2e -- --grep "task flow response" && npm --prefix crates/harness-symphony/web-ui run build`

Expected: PASS.

- [ ] **Step 5: Commit the frontend contract**

```bash
git add crates/harness-symphony/web-ui/src/features/symphony/types.ts crates/harness-symphony/web-ui/src/features/symphony/api.ts crates/harness-symphony/web-ui/tests/board.spec.ts
git commit -m "test: validate Symphony task flow responses"
```

### Task 4: Render The Compact Horizontal Timeline

**Files:**
- Create: `crates/harness-symphony/web-ui/src/features/symphony/active-task-flow.tsx`
- Modify: `crates/harness-symphony/web-ui/src/main.tsx`
- Test: `crates/harness-symphony/web-ui/tests/board.spec.ts`

- [ ] **Step 1: Add failing Playwright tests for idle and active flow**

Update the board route fixtures to include `task_flow`. Assert the lifecycle region precedes the command status rail, always exposes seven ordered labels, shows a neutral idle message, and marks Agent as current for an active fixture.

```ts
const flow = page.getByRole("region", { name: "Active task lifecycle" });
await expect(flow).toContainText("No task is currently running");
await expect(flow.getByRole("listitem")).toHaveCount(7);
await expect(flow.getByRole("listitem").nth(1)).toHaveAttribute("aria-current", "step");
```

- [ ] **Step 2: Run the focused E2E test and verify RED**

Run: `npm --prefix crates/harness-symphony/web-ui run e2e -- --grep "active task lifecycle"`

Expected: FAIL because the region is absent.

- [ ] **Step 3: Implement `ActiveTaskFlow`**

Use `Check`, `Circle`, `Loader2`, and `AlertTriangle` from lucide-react. Render a `Card` with a header, task identity/message, and an ordered list. Use `aria-current="step"` for the current node, visible state text for screen readers, and `motion-safe:animate-pulse` only on the current marker.

```tsx
export function ActiveTaskFlow({ flow, stale = false }: { flow: TaskFlow | null; stale?: boolean }) {
  const steps = flow?.steps ?? idleSteps;
  return (
    <Card asChild={false} className="overflow-hidden rounded-xl border bg-card p-3 lg:p-4">
      <section aria-label="Active task lifecycle">
        <header className="flex min-w-0 items-center justify-between gap-3">
          <div className="min-w-0">
            <p className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">Task lifecycle</p>
            <p className="truncate text-sm font-bold">{flow ? `${flow.story_id} · ${flow.title}` : "No task is currently running"}</p>
          </div>
          <span className="text-xs font-semibold text-muted-foreground">{flow?.message ?? "Symphony is idle"}</span>
        </header>
        <ol className="scrollbar-none mt-3 flex min-w-max items-start overflow-x-auto" aria-label="Task lifecycle steps">
          {steps.map((step, index) => <FlowStep key={step.id} step={step} index={index} />)}
        </ol>
        {stale ? <p role="status">Unable to refresh; showing the last known task state.</p> : null}
      </section>
    </Card>
  );
}
```

Keep connectors inside each list item so the row remains continuous. Use existing semantic colors and `min-w` values that allow bounded horizontal scrolling without page-level overflow.

- [ ] **Step 4: Integrate above `SummaryStrip` and preserve stale state**

Store `taskFlow` and `taskFlowStale` in `App`. Successful loads replace the model and clear stale. A silent refresh failure keeps the previous model and sets stale; an initial failure renders idle plus the existing board error. Pass the flow action to the existing recovery callback rather than duplicating endpoint logic.

- [ ] **Step 5: Run focused E2E and build**

Run: `npm --prefix crates/harness-symphony/web-ui run e2e -- --grep "active task lifecycle" && npm --prefix crates/harness-symphony/web-ui run build`

Expected: PASS.

- [ ] **Step 6: Commit the timeline component**

```bash
git add crates/harness-symphony/web-ui/src/features/symphony/active-task-flow.tsx crates/harness-symphony/web-ui/src/main.tsx crates/harness-symphony/web-ui/tests/board.spec.ts
git commit -m "feat: show active task lifecycle above Symphony board"
```

### Task 5: Prove Failure, Review, Done, And Responsive Behavior

**Files:**
- Modify: `crates/harness-symphony/web-ui/tests/board.spec.ts`
- Modify: `docs/stories/US-090-symphony-active-task-flow.md`

- [ ] **Step 1: Add remaining failing browser scenarios**

Add fixtures and assertions for validation failure with recovery, waiting for merge, waiting for sync, done, stale refresh, reduced motion, and a 390px viewport. For narrow screens assert no page-level horizontal overflow while the lifecycle list itself scrolls and all seven accessible labels remain present.

```ts
await page.setViewportSize({ width: 390, height: 844 });
await expectPageNoHorizontalOverflow(page);
await expect(flow.getByRole("listitem")).toHaveCount(7);
expect(await flow.getByRole("list").evaluate(el => el.scrollWidth > el.clientWidth)).toBe(true);
```

- [ ] **Step 2: Run scenarios and verify RED where coverage is missing**

Run: `npm --prefix crates/harness-symphony/web-ui run e2e -- --grep "active task lifecycle"`

Expected: at least one new scenario FAIL before the final CSS/state adjustments.

- [ ] **Step 3: Make the smallest visual and state adjustments**

Adjust only the new component and its integration: semantic state classes, short responsive labels, recovery button wiring, stale copy, and reduced-motion classes. Do not redesign board columns or task cards.

- [ ] **Step 4: Run full validation**

Run:

```bash
cargo fmt --check
cargo test -p harness-symphony
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix crates/harness-symphony/web-ui run build
npm --prefix crates/harness-symphony/web-ui run e2e
npm --prefix crates/harness-symphony/web-ui run desktop:smoke
git diff --check
scripts/bin/harness-cli story verify US-090
```

Expected: all commands PASS. Record exact counts and any unavailable platform proof in the story evidence.

- [ ] **Step 5: Update durable proof and story evidence**

After proof passes, update the story packet with commands and counts, then run:

```bash
scripts/bin/harness-cli story update --id US-090 --status implemented --unit 1 --integration 1 --e2e 1 --platform 1 --evidence "Rust lifecycle/API tests, strict TypeScript parser tests, Playwright lifecycle scenarios, Web build, Electron smoke, workspace tests, clippy, and diff check passed."
```

- [ ] **Step 6: Commit the completed proof**

```bash
git add crates/harness-symphony/web-ui/tests/board.spec.ts docs/stories/US-090-symphony-active-task-flow.md
git commit -m "test: verify Symphony active task flow"
```
