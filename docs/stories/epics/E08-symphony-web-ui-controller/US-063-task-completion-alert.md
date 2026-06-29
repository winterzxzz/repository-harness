# US-063 Task Completion Alert

## Status

planned

## Lane

normal

## Product Contract

When an active Symphony task completes successfully from the Web UI, the shared
browser/Electron controller should show a small, non-blocking alert so the user
can see that the run finished without needing to scan the board manually.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`

## Acceptance Criteria

- When a Web UI-started active task finishes successfully and the board updates
  out of `In Progress`, the UI shows a small completion alert.
- The alert identifies the completed task in simple user-facing language and
  offers a clear path back to the task detail or review surface when review is
  available.
- The alert is transient and non-blocking: it must not interrupt event polling,
  board refresh, PR creation, review, merge marking, sync approval, or task
  selection.
- The alert is shown only for newly observed completion in the current UI
  session, not for already completed tasks loaded on initial page render.
- Failed, interrupted, or needs-attention runs do not use the success alert;
  existing needs-attention messaging remains the failure path.
- The behavior works in the browser UI and Electron desktop shell because it
  lives in the shared React Web UI surface.

## Design Notes

- Commands: `harness-symphony web`; Electron continues to package the same Web
  UI assets.
- Queries: no new Harness queries expected.
- API: no new API expected; derive the alert from existing board/run polling
  state unless implementation discovers a gap.
- Tables: none.
- Domain rules: the alert is presentation-only and must not affect Harness
  stories, Symphony runs, review state, PR state, sync state, or durable
  records.
- UI surfaces: active task board/detail state in
  `crates/harness-symphony/web-ui`.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-063 --unit 1 --integration 1 --e2e 1 --platform 1`.

| Layer | Expected proof |
| --- | --- |
| Unit | Component or state-transition test covers showing the alert only for newly observed successful completion and suppressing it on initial completed board load. |
| Integration | Web UI build proves the shared React surface compiles without backend/API changes unless a scoped API update is explicitly justified. |
| E2E | Playwright covers an active task completion transition and verifies the alert appears without blocking task detail/review interaction. |
| Platform | Electron smoke/build proves the shared UI still packages with the alert behavior. |
| Release | Not required. |

## Harness Delta

No process change.

## Evidence

Add commands, reports, screenshots, or links after validation exists.
