# 0008 Request Changes Feedback Artifacts

Date: 2026-07-10

## Status

Accepted

## Context

Symphony places completed agent work in the Ready bucket so a user can inspect
the result before approval and sync. The existing Reject action records a text
reason on the completed run but does not immediately rerun the same story, does
not accept image evidence, and does not provide a complete feedback contract to
the next agent attempt.

Image feedback introduces filesystem retention, upload validation, request-size
limits, path safety, and agent-access concerns. The feedback must remain useful
without committing screenshots or other local evidence to Git.

## Decision

Add a `Request changes` operation for eligible Ready runs.

- A non-empty reason is required and limited to 2,000 characters.
- Up to three optional PNG, JPEG, or WebP files are accepted, with a 5 MB limit
  per file.
- The browser sends the reason and files in one bounded multipart request.
- The backend validates the full request before changing run state.
- Valid feedback creates a new run for the same story and moves the task from
  Ready to Active without adding another board bucket.
- The prior run is retained and marked rejected only after the replacement run
  is prepared successfully.
- Feedback is stored as local run artifacts under
  `.harness/runs/<new_run_id>/feedback/` and is never committed to Git.
- The new run contract and agent prompt include the feedback reason and evidence
  paths.
- Done tasks cannot use this operation; accepted work requires a follow-up
  story instead of reopening historical completion.

## Alternatives Considered

1. Encode images as base64 JSON. Rejected because it expands request size,
   duplicates binary data in memory, and makes the current HTTP boundary harder
   to secure.
2. Upload files through a separate endpoint before requesting changes. Rejected
   because it introduces orphan cleanup and a two-step user transaction without
   enough benefit for a local controller.
3. Add a `Changes Requested` board bucket. Rejected because the existing
   Ready-to-Active transition already communicates that a replacement run is in
   progress.
4. Create a new story for every rejected result. Rejected because the feedback
   is another attempt at the same accepted requirement, not a separate work
   item.

## Consequences

Positive:

- Users can give actionable visual feedback without leaving the task detail
  flow.
- Retry history, evidence, and agent attempts remain inspectable.
- The four-bucket board model stays stable.
- Evidence retention follows existing local run-artifact cleanup.

Tradeoffs:

- The HTTP server must support bounded body reads and multipart parsing.
- Run preparation needs a feedback-aware contract and cleanup path.
- Image understanding still depends on the selected agent adapter's ability to
  inspect local files.

## Follow-Up

- Validate request-size limits and file signatures with deterministic Rust
  tests.
- Cover Ready-to-Active request changes with browser E2E and desktop smoke.
- Keep future support for video, arbitrary attachments, and Done-task reopen out
  of scope until real demand exists.
