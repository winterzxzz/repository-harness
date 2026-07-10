# US-084 Request Changes With Image Evidence

## Status

implemented

## Lane

high-risk

## Current Behavior

Eligible completed agent work in the Ready bucket now exposes an inline
`Request changes` form. The user must enter a reason and may attach up to three
validated PNG, JPEG, or WebP images before Symphony prepares and starts a
replacement run for the same story.

The local HTTP server uses a bounded binary reader and multipart parser instead
of the legacy fixed 8 KB string buffer. Feedback is written to durable root and
replacement-worktree run directories, referenced by the run contract and agent
prompt, and served back only through scoped generated evidence paths.

## Target Behavior

A user reviewing an eligible Ready task can enter a required change reason,
attach up to three validated screenshots, and submit `Request changes`.
Symphony prepares a replacement run for the same story, stores feedback as
local run artifacts, preserves the old run, passes feedback paths to the agent,
and moves the task directly to Active.

## Affected Users

- Product owner reviewing completed local agent work.
- Developer operating the Symphony Web UI.
- Agent adapter consuming the replacement run contract.

## Affected Product Docs

- `docs/product/symphony-web-ui-controller.md`
- `docs/decisions/0008-request-changes-feedback-artifacts.md`
- `docs/superpowers/specs/2026-07-10-request-changes-with-image-evidence-design.md`

## Non-Goals

- Reopening Done tasks.
- Adding another board bucket or story status.
- Accepting non-image attachments, SVG, PDF, or video.
- Uploading evidence to cloud storage or Git.
- Redesigning unrelated board, PR, sync, or retention behavior.

## Acceptance Criteria

- Ready task detail exposes `Request changes` with mandatory reason and optional
  image selection.
- The UI accepts no more than three PNG/JPEG/WebP images of at most 5 MB each.
- Invalid feedback never starts the agent or changes the source run.
- Successful feedback creates a new run for the same story and moves the task
  to Active.
- The prior run remains available and is marked rejected with the user reason.
- Feedback artifacts exist under the replacement run and appear in its contract
  and agent prompt.
- Done, non-current, non-Ready, and active-conflict requests are refused.
- Feedback follows run compaction and is never added to Git.
