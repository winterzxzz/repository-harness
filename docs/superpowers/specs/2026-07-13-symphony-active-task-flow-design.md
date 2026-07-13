# Symphony Active Task Flow Design

## Intent

The Symphony controller should show the operator where the current task is in
its end-to-end lifecycle without requiring them to open task details or infer
progress from board buckets and logs.

The flow is a compact horizontal timeline above the existing command status
rail and board. It remains visible when Symphony is idle so the page layout is
stable and the absence of active work is explicit.

## Approved Experience

The flow contains seven ordered steps:

1. Start
2. Agent
3. Validation
4. Pull request
5. Review and merge
6. Sync
7. Done

The component shows the current story ID and title when a task owns the active
lifecycle. Completed steps use a success treatment and check mark, the current
step uses the Symphony blue accent and restrained motion, future steps remain
neutral, and a failed step uses the destructive treatment with the existing
recovery action when one is available.

When no task is active or awaiting review, the same seven-step flow stays
visible in a neutral state and says that no task is currently running. The UI
must not show a fabricated completion percentage or estimated duration.

## Lifecycle Ownership

The flow follows one task through both automated and human-owned phases. An
`In Progress` task remains in the flow while the agent and validation stages
run. A `Review` task remains in the flow while its pull request is created,
reviewed, and merged. The task reaches Done only after the accepted changes are
synced according to Symphony's existing rules.

`Needs Attention` does not become an eighth lifecycle step. The flow stops at
the step whose work failed, marks that step as failed, and presents the concise
failure reason and recovery action supplied by Symphony. This preserves the
normal mental model while making interruption visible.

## Approaches Considered

### Compact horizontal timeline

Selected. It keeps the board above the fold, works with Symphony's existing
command-center hierarchy, and gives every lifecycle stage a stable position.

### Context-rich status rail

Rejected for the initial version. It offers more room for event summaries but
duplicates information already available in task detail and consumes more
vertical space above the board.

### Vertical stepper

Rejected. It is easy to expand with detail but pushes the board too far down
and conflicts with the requirement that the flow sit compactly at the top of
the controller.

## Architecture

The Web UI receives a normalized task-flow model rather than reconstructing
business state from human-readable log messages. The model contains the owning
story, overall flow state, ordered steps, the current or failed step, and an
optional concise status message and recovery action.

The backend derives this model from the existing sources of truth:

- board and run status
- Codex lifecycle events
- result validation
- pull-request status
- merge status
- sync status

The derivation belongs beside Symphony's existing board/review state logic so
the Web UI does not create a second workflow model. Existing board states and
active-run locking remain unchanged.

The frontend adds a focused `ActiveTaskFlow` component above `SummaryStrip`.
It renders the normalized model, owns no polling loop of its own, and refreshes
with the board data already loaded by the controller.

## Data Contract

The board response should include one top-level optional `task_flow` object.
Its conceptual shape is:

```text
task_flow:
  story_id: string | null
  title: string | null
  state: idle | active | waiting | failed | done
  current_step: start | agent | validation | pr | review | sync | done | null
  message: string
  steps:
    - id: lifecycle step id
      state: pending | current | complete | failed
  recovery_action: existing recovery action or null
```

The exact serialized names may follow established Rust and TypeScript naming
patterns, but the frontend parser must validate the response as it does other
board data.

## Responsive And Visual Behavior

On desktop, all seven labels appear in one row. The connector and nodes form a
single continuous track, with the status message on a compact second line.
Motion is limited to the current node and disabled under reduced-motion user
preferences.

On narrow screens, the timeline remains one row and becomes horizontally
scrollable with short labels. Steps must not wrap into an ambiguous second
row, shrink into unreadable dots, or disappear. Focus and screen-reader order
must match lifecycle order.

Color is never the only status signal: completed steps have a check mark, the
current step has an explicit current-state label, and failed steps include an
error icon and text.

## Error And Stale Data Handling

- A known task failure marks the derived failed step and preserves its evidence.
- A recoverable failure exposes Symphony's existing guarded recovery action.
- If refresh fails after valid data was shown, the UI preserves the last model
  and identifies it as temporarily stale.
- If no valid flow model has loaded, the component renders the neutral idle
  skeleton rather than guessing a stage.
- Unknown future states degrade to a neutral message and must not be shown as
  completed.

## Validation

- Rust unit tests cover lifecycle derivation for idle, agent execution,
  validation failure, PR creation, waiting for merge, sync, and done.
- TypeScript parser tests reject malformed task-flow payloads.
- Component or Playwright coverage proves idle, active, failed, review, and
  done presentations.
- Browser E2E proves the flow remains above the status rail and board, exposes
  accessible lifecycle order, and stays usable at narrow viewport widths.
- Existing board, task detail, review, recovery, and sync tests remain green.
- Release checks include Web UI build, relevant E2E coverage, Rust formatting,
  targeted tests, workspace tests, clippy, and `git diff --check`.

## Scope Boundaries

- No dependency graph between stories.
- No concurrent-run UI or changes to the single-active-run rule.
- No estimated completion percentage or time remaining.
- No new task runner, lifecycle database, or source of truth.
- No replacement for task detail, logs, review evidence, or raw artifacts.
- No unrelated board redesign.
