# US-090 Symphony Active Task Lifecycle Flow

## Status

planned

## Lane

normal

## Product Contract

The Symphony controller always shows a compact horizontal lifecycle flow above
the board. It follows the current task from start through agent execution,
validation, pull request, review and merge, sync, and done using runtime state
owned by Symphony. When no task owns the lifecycle, the same flow remains
visible in a neutral idle state.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`
- `docs/superpowers/specs/2026-07-13-symphony-active-task-flow-design.md`

## Acceptance Criteria

- The controller renders the seven-step horizontal flow above the existing
  command status rail and board.
- Idle, active, waiting, failed, and completed presentations come from a
  normalized backend model derived from the run's durable `current_stage`.
- A failed task marks the lifecycle step that failed and exposes the existing
  concise recovery action without inventing a separate error step.
- The flow uses text or icons in addition to color, respects reduced motion,
  and remains readable through horizontal scrolling on narrow screens.
- The UI never invents a completion percentage or estimated duration.
- Existing board, detail, review, recovery, and sync behavior remains intact.

## Design Notes

- API: extend the board response with one normalized `task_flow` object.
- Domain rules: `run_state.current_stage` is authoritative from Start through
  Done; failures and cancellations retain the stage where execution stopped.
  The frontend does not parse human-readable logs to create lifecycle state.
- UI surfaces: add a focused `ActiveTaskFlow` above `SummaryStrip`.
- Responsive behavior: preserve one ordered row; allow bounded horizontal
  scrolling instead of wrapping or hiding steps.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id <id> --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | Rust lifecycle derivation and TypeScript response parser tests |
| Integration | Board API returns normalized idle, active, failure, review, sync, and done flows |
| E2E | Playwright covers visual order, status semantics, recovery affordance, and narrow viewport behavior |
| Platform | Web build plus Electron desktop smoke |
| Release | Rust fmt, targeted/workspace tests, clippy, Web UI build/E2E, and `git diff --check` |

## Harness Delta

None. This is a source-repo Symphony controller feature and does not change the
fresh-install Harness template contract.

## Evidence

- `cargo test -p harness-symphony` — 204 passed, 0 failed.
- `cargo test --workspace` — Harness Bench 20, Harness CLI 43, and Symphony
  204 tests passed; doc tests passed.
- `cargo clippy --workspace -- -D warnings` — passed.
- `npm --prefix crates/harness-symphony/web-ui run build` — passed.
- `npm --prefix crates/harness-symphony/web-ui run e2e` — 38 passed, 0 failed,
  including idle, active, failure recovery, narrow viewport, overflow, reduced
  motion, review, and done regressions.
- `node .agents/skills/impeccable/scripts/detect.mjs --json ...` — no
  deterministic design-quality findings in the changed UI files.
- Electron desktop smoke is unavailable from a linked git worktree because its
  root-discovery assertion resolves the shared checkout instead of the worktree;
  Web build, backend API tests, and browser E2E passed.
