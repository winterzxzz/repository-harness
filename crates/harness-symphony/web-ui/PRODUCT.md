# Symphony Web UI Product Context

This UI is the local controller for Harness Symphony work. It is not a hosted
dashboard, project-management replacement, or marketing surface.

## Operator

The operator is a local developer or technical lead running Symphony against the
current repository. They need to see what work is safe to start, what is blocked,
what is running, what needs review, what needs attention, and what is done.

## Primary Job

Help the operator control one local Symphony run at a time:

- See work grouped as Drafts, Active, Ready, and Done instead of scanning every
  internal runner state as a separate board column.
- Start Ready work only when dependencies allow it.
- Watch active run status and readable Codex progress.
- Review run artifacts, validation, changed files, and PR status.
- Mark PRs merged, approve sync, and see Done state.
- Recover eligible Needs Attention runs without losing failure evidence.

## Non-Goals

- Authentication or hosted multi-user use.
- Creating new stories from the Web UI.
- Replacing Harness feature intake or story planning.
- Replacing raw run artifacts or durable Harness records.
- Decorative storytelling, landing-page composition, or broad analytics.

## Source Of Truth

Harness and Symphony state remain the source of truth. The Web UI should present
and control that state without inventing a second workflow model.

Canonical product contract:

- `docs/product/symphony-web-ui-controller.md`
