# Symphony Board Status Ownership Design

## Problem

The Symphony controller currently presents four user-facing buckets as
`Drafts`, `Active`, `Ready`, and `Done`. These labels describe storage or
workflow conditions, but they do not consistently tell an operator who owns the
next action. In particular, `Ready` can be read as ready to start even though
that column contains completed agent work waiting for human review.

The controller already has a separate seven-step active-task lifecycle. That
flow is useful and must remain the only detailed progress visualization.

## Outcome

Make the four status surfaces communicate ownership at a glance:

1. `Planned` — work that has not started.
2. `Agent working` — work owned by Codex or requiring recovery before Codex can
   continue.
3. `Human review` — completed agent work waiting for a human decision, merge,
   or sync approval.
4. `Done` — accepted and synchronized work.

The same presentation labels appear in the command status rail, board column
headers, sidebar navigation, accessible names, and empty-state copy.

## Scope

This is a source-repository Symphony Web UI change. It is not a Harness template
defect and does not change the fresh-install payload.

In scope:

- Replace user-facing bucket labels with ownership-oriented labels.
- Add concise supporting copy to board column headers so each column explains
  its next-action owner.
- Preserve an icon, task count, and subtle semantic color for each status.
- Keep `Blocked` visibly exceptional inside `Planned`.
- Keep `Needs Attention` visibly exceptional inside `Agent working` and retain
  its existing failure explanation and guarded recovery action.
- Keep desktop, narrow-screen, dark-mode, reduced-motion, and accessible-name
  behavior coherent.

Out of scope:

- Changing the seven-step active-task lifecycle or its placement.
- Changing internal `BoardBucket` keys, Symphony task states, bucket grouping,
  backend derivation, APIs, database schema, runner behavior, dependencies,
  transition rules, or task-card actions.
- Adding a fifth status column or a second pipeline visualization.
- Translating the rest of the controller.

## Approaches Considered

### 1. Rename only

Change visible labels and leave the surrounding status presentation unchanged.
This has the smallest patch but does not deliver enough visual clarity.

### 2. Semantic ownership headers — selected

Use ownership labels, existing semantic icons, counts, subtle color, and one
short explanation per status. This makes the board scannable without competing
with the detailed lifecycle.

### 3. Connected four-stage pipeline

Render the four statuses as a connected sequence. This resembles a state
machine, but it duplicates the existing seven-step lifecycle and creates two
competing progress models.

## Architecture

Keep the existing internal bucket values (`Drafts`, `Active`, `Ready`, `Done`)
as stable grouping keys. Add one presentation metadata map keyed by
`BoardBucket` that provides:

- visible label;
- concise ownership description;
- icon;
- tone or semantic styling.

`SummaryStrip`, `BoardGrid`, and sidebar navigation consume this metadata rather
than repeating user-facing strings. `bucketForItem` remains unchanged:

- internal `Ready` and `Blocked` tasks map to `Drafts` / visible `Planned`;
- internal `In Progress` and `Needs Attention` tasks map to `Active` / visible
  `Agent working`;
- internal `Review` maps to `Ready` / visible `Human review`;
- internal `Done` maps to `Done`.

The frontend must not infer status from human-readable logs. Existing backend
state remains authoritative.

## UI Behavior

Each board column header shows its semantic icon, ownership label, one-line
description, and count. Descriptions are:

- Planned: `Ready to start · blocked work stays visible`.
- Agent working: `Codex owns the next action`.
- Human review: `Waiting for your decision`.
- Done: `Accepted and synchronized`.

The command status rail uses equivalent wording and may include the active task
identifier in the Agent working value exactly as it does today. On narrow
screens, existing horizontal/vertical scrolling rules remain unchanged and the
new copy must remain bounded.

The existing red failure treatment and recovery controls remain authoritative
for `Needs Attention`; semantic blue styling for the parent column must not
hide the exception. The existing blocked treatment remains visible inside
Planned.

## Accessibility And Error Handling

- Accessible column and task-list names use the visible ownership labels.
- Icons remain supplementary; text and state badges carry meaning without
  color.
- Active animation continues to respect reduced motion.
- Existing stale-data, failure, empty-state, and recovery behavior is unchanged.
- Long labels and descriptions remain bounded without horizontal card overflow.

## Validation

- Add focused tests for the presentation mapping and internal-to-visible bucket
  grouping.
- Update Playwright expectations from the old labels to the ownership labels.
- Assert the seven lifecycle steps and their order are unchanged.
- Cover `Blocked` in Planned and `Needs Attention` in Agent working.
- Verify desktop and narrow viewport layouts have no unintended overflow.
- Run Web UI build and E2E, relevant Rust tests, workspace formatting/clippy,
  desktop smoke when available, and `git diff --check`.
