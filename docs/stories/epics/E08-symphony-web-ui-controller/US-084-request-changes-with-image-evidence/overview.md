# US-084 Request Changes With Image Evidence

## Status

planned

## Lane

high-risk

## Current Behavior

Completed agent work appears in the Ready bucket. The review panel can record a
text rejection on the completed run, but it does not immediately start a new
attempt, accept visual evidence, or pass structured feedback into the next run
contract.

The local HTTP server reads a single fixed 8 KB buffer, which is not a safe or
complete upload boundary for binary screenshots.

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
