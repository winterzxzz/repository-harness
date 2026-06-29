# US-062 Task Detail Close Confetti

## Status

implemented

## Lane

normal

## Product Contract

When a user closes the floating task detail popup with its `X` button, the Web
UI should play a brief, polished confetti effect that gives visual feedback
without delaying close behavior, changing task state, or adding a second source
of truth for Symphony data.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`

## Acceptance Criteria

- Clicking the task detail popup `X` button closes the popup and starts a
  short confetti animation near the close interaction.
- The effect is presentation-only: it does not change selected task state
  beyond the existing close behavior, board state, run state, APIs, or durable
  Harness records.
- The animation is bounded and cleans up any timers, DOM nodes, canvas state, or
  listeners after it completes.
- Users with reduced motion preferences get a non-animated or suppressed effect
  while preserving the close behavior.
- The effect works in the browser UI and the Electron desktop shell because it
  lives in the shared React Web UI surface.
- Existing board selection, popup close, detail actions, review content, and log
  rendering keep passing.

## Design Notes

- Commands: `harness-symphony web`; Electron continues to package the same Web
  UI assets.
- Queries: none.
- API: none.
- Tables: none.
- Domain rules: confetti is purely local UI feedback and must not affect
  Harness stories, Symphony runs, review state, PR state, or sync state.
- UI surfaces: close button in the floating task detail popup in
  `crates/harness-symphony/web-ui`.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-062 --unit 1 --integration 1 --e2e 1 --platform 1`.

| Layer | Expected proof |
| --- | --- |
| Unit | React/component or utility test covers triggering the confetti effect, cleanup, and reduced-motion behavior if the implementation extracts logic. |
| Integration | Web UI build proves the shared React surface compiles with the effect and no backend/API changes are required. |
| E2E | Playwright clicks a task, clicks the popup `X` button, verifies the popup closes, and verifies the confetti host/effect appears or is invoked without breaking board interaction. |
| Platform | Electron smoke/build proves the shared Web UI still packages with the confetti dependency or implementation. |
| Release | Not required. |

## Harness Delta

No process change.

## Evidence

- Implemented a shared React Web UI close-button confetti burst that starts
  when the floating task detail popup `X` button is clicked, then closes the
  popup immediately without API, board-state, run-state, or durable-record
  changes.
- Added bounded cleanup for transient confetti bursts and reduced-motion
  suppression.
- Added Playwright coverage for close behavior, confetti rendering and cleanup,
  and reduced-motion close behavior.
- Validation passed:
  `npm --prefix crates/harness-symphony/web-ui run build`;
  `npm --prefix crates/harness-symphony/web-ui run e2e`;
  `npm --prefix crates/harness-symphony/web-ui run desktop:smoke`;
  `cargo test --workspace`;
  `cargo fmt --check`;
  `cargo clippy --workspace -- -D warnings`;
  `git diff --check`.
