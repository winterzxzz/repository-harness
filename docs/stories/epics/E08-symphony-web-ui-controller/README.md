# E08 Symphony Web UI Controller

## Goal

Build the local browser controller described in
`docs/product/symphony-web-ui-controller.md` without creating a second source of
truth for Harness or Symphony state.

## Product Contract

The Web UI is a local-only controller over existing Harness stories and
Symphony runs. It must derive task hierarchy, dependency blocking, board state,
run progress, review readiness, PR state, and sync state from Harness and
Symphony records.

## Stories

1. `US-047` - Dependency-aware board state foundation.
2. `US-048` - Local web backend command and API contract.
3. `US-049` - Browser board and task detail UI.
4. `US-050` - Run start, active-run enforcement, and Codex event streaming.
5. `US-051` - Review surface for result artifacts, changesets, PR state, and
   logs.
6. `US-052` - Sync approval flow and Done transition.
7. `US-053` - Browser E2E validation for the MVP workflow.
8. `US-056` - Simplified Kanban-first controller reference revamp.
9. `US-057` - Dependency graph sidebar view.
10. `US-058` - Scrollable board columns.
11. `US-059` - Electron desktop shell.
12. `US-060` - Human-readable chat logs.
13. `US-061` - FrankenTUI app server log TUI.
14. `US-062` - Confetti close effect for the task detail popup.
15. `US-063` - Small completion alert when an active task finishes.

## Exit Criteria

- `harness-symphony web` serves a local unauthenticated browser UI.
- The UI shows all Harness stories with Ready, Blocked, In Progress, Review,
  Needs Attention, and Done states derived from durable data.
- A user can start exactly one ready task, watch live Codex App Server events,
  review completed artifacts and PR state, approve sync after merge, and see
  the task move to Done.
- Dependency cycles are detected and shown as task breakdown problems.
- Browser-level validation proves the board, task detail, event stream, review,
  and sync workflows.
- The same controller can be rebuilt into an Electron desktop shell without
  changing Harness or Symphony state ownership.
- Technical maintainers can optionally inspect local app-server logs from a
  terminal TUI without replacing the browser or Electron review surfaces.
