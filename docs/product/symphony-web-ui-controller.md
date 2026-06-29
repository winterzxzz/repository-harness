# Symphony Web UI Controller

## Purpose

The Symphony Web UI Controller gives non-technical users a local browser surface
for controlling Harness Symphony tasks without needing to operate the CLI
directly.

The UI is a controller for existing Harness and Symphony concepts. It should not
become a second task runner, a second feature intake system, or a separate source
of truth.

## Users

- Non-technical task owners who need to start, watch, review, and approve
  Symphony work.
- Technical maintainers who need to inspect run details, logs, validation
  evidence, PR state, and sync state from the same surface.

## Product Principles

- Use simple user-facing labels. Avoid exposing Harness terms unless a detail
  panel needs them.
- Present run communication as readable conversation and progress summaries
  first, while keeping raw artifacts available for technical review.
- Keep all tasks visible, but block unsafe actions when dependencies or active
  run rules prevent work from starting.
- Make hierarchy explicit so users can understand how feature intake breaks a
  larger request into executable work.
- Keep the MVP local-only and unauthenticated.
- Allow only one active task at a time in the MVP.
- Preserve the existing Symphony workflow: run workspace, Codex App Server
  adapter, result artifacts, PR review, merged PR, and sync.

## Task Source

Tasks come from Harness stories created during feature intake.

Feature intake is responsible for producing:

- Task hierarchy.
- Task dependencies.
- Runnable task boundaries.
- Validation expectations.

The Web UI does not create tasks in the MVP.

## Dependency Model

Dependencies live in Harness because they are produced during feature intake.

A task dependency means one task cannot move into `In Progress` until every
blocking task is `Done`.

The dependency graph must support:

- Listing direct blockers for a task.
- Listing tasks unblocked by a completed task.
- Deriving `Ready` and `Blocked` status.
- Detecting cycles before tasks are presented as runnable.
- Explaining why a blocked task is blocked in simple language.

Cycle detection is required. A cycle is a product planning error and should be
shown as a task breakdown problem, not as a user action problem.

## Board States

The primary board states are:

| State | Meaning |
| --- | --- |
| Ready | The task has no incomplete blockers and can be selected. |
| Blocked | The task has incomplete blockers or a dependency cycle. |
| In Progress | The user selected the task and Symphony is running it. |
| Review | A run completed, `RESULT.json` exists, and a PR has been created. |
| Needs Attention | A run failed, was interrupted, or cannot create required review artifacts. |
| Done | The PR was merged and Symphony sync applied the accepted changeset. |

## Main Workflow

1. User opens the local Web UI.
2. UI shows task hierarchy and board states.
3. User clicks a task and inspects the floating task detail popup without
   losing the board context.
4. User starts a `Ready` task from the popup.
5. The task moves to `In Progress`.
6. Entering `In Progress` starts execution like `harness-symphony run`.
7. UI shows live Codex App Server events for the active run.
8. When Codex emits `turn/completed` with completed status and required
   artifacts validate, Symphony creates a PR.
9. The task moves to `Review`.
10. User reviews summary, result, changeset, validation evidence, PR status, and
   logs.
11. After the PR is merged, the user approves sync from the UI.
12. UI runs Symphony sync.
13. The task moves to `Done`.

## Failure Workflow

If Codex fails, the run is interrupted, required artifacts are missing, PR
creation fails, or validation fails, the task moves to `Needs Attention`.

`Needs Attention` must show:

- What failed.
- The last observed Codex event or error.
- Links to run artifacts when present.
- Suggested next action.
- Retry controls when retry is safe.

## Review Surface

The review screen should expose enough information for the user to make an
approval decision without leaving the Web UI.

It should include:

- Task summary.
- Run outcome from `RESULT.json`.
- Validation evidence.
- Changed files.
- Human-readable changeset preview.
- PR link and merge status.
- Codex event log.
- Human-readable chat and progress log derived from Codex events.
- Run summary.
- Approve/sync action after the PR is merged.
- Retry or mark-needs-attention actions when review artifacts are incomplete.

Raw artifacts should remain accessible from the review surface.

## Codex Event Source

The existing Symphony Codex adapter writes Codex App Server JSON-RPC events to:

```text
.harness/runs/<run_id>/APP_SERVER_EVENTS.jsonl
```

The Web UI should stream or tail this file for the active run.

Useful event types include:

- `thread/started`
- `turn/started`
- `item/agentMessage/delta`
- `item/completed`
- `turn/diff/updated`
- `thread/status/changed`
- `turn/completed`

`turn/completed` with completed status is required before moving to `Review`.

## Local Web Boundary

The MVP should be local-only and no-auth.

The expected control surface is:

```text
harness-symphony web
```

The backend should expose local APIs for board data, task details, run start,
event streaming, review state, PR status, and sync.

The frontend should use Vite, React, and shadcn components.

## Desktop Boundary

The desktop app is a packaged shell around the same local Web UI controller. It
starts a loopback `harness-symphony web` backend, loads the existing React UI,
and keeps the existing `/api/*` routes as the contract between renderer and
backend.

The desktop shell must not fork task execution, durable state, review behavior,
or sync behavior. Rebuilding the desktop app should include the latest Web UI
build output and the current `harness-symphony` backend binary.

## Non-Goals

- User authentication.
- Hosted team deployment.
- Multiple active tasks.
- Task creation from the Web UI.
- External issue tracker adapters.
- Replacing Harness feature intake.
- Replacing Symphony run isolation.
- Desktop signing, notarization, auto-update, and installer distribution for
  the first desktop MVP.

## MVP Implementation Decisions

- Dependency edges are stored in Harness `story_dependency` records.
- Task hierarchy is stored in Harness `story_hierarchy` records.
- PR merge status is entered manually for the MVP through the local Web UI.
- Codex events are exposed as a polling tail snapshot from
  `APP_SERVER_EVENTS.jsonl` through the local Web API.
- The primary UI should summarize Codex events into readable chat/progress
  entries; raw `APP_SERVER_EVENTS.jsonl` remains available for debugging.

## Validation Expectations

Implementation stories should include proof for:

- Dependency graph ready/blocked derivation.
- Dependency cycle detection.
- Single-active-task enforcement.
- `Ready` to `In Progress` transition.
- Codex event streaming from `APP_SERVER_EVENTS.jsonl`.
- `turn/completed` plus valid `RESULT.json` transition to `Review`.
- Failed run transition to `Needs Attention`.
- PR merged plus sync transition to `Done`.
- Browser UI flow through Playwright or equivalent.
