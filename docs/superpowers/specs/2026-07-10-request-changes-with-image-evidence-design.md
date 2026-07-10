# Request Changes With Image Evidence Design

## Problem

The Symphony Web UI exposes four board buckets: Drafts, Active, Ready, and Done.
Ready contains completed agent work awaiting human acceptance. When the result
is not satisfactory, the user needs to explain what must change, attach visual
evidence, and rerun the same story without creating a new task or losing prior
run history.

The current Reject action only records a short text reason on the existing run.
It does not upload images, create a replacement run immediately, or provide the
feedback to the next agent through a durable contract.

## Desired Outcome

From an eligible Ready task, a user can submit a required reason and up to three
optional evidence images. Symphony validates the request, prepares a new run for
the same story, stores the feedback as local artifacts, passes it to the agent,
marks the old run rejected, and moves the board item directly to Active.

## Confirmed Product Decisions

- Keep the existing four board buckets.
- Request changes is available only before a task reaches Done.
- The same story is reused; no follow-up story is created for a rejected Ready
  result.
- The textual reason is required and limited to 2,000 characters.
- Evidence images are optional, with at most three files and 5 MB per file.
- Accepted formats are PNG, JPEG, and WebP.
- Feedback files remain local run artifacts and are never committed to Git.
- Any invalid file or failed preparation rejects the whole operation and does
  not start the agent.

## Lifecycle

```text
Drafts -> Active -> Ready
                    | approve/sync -> Done
                    | request changes
                    v
                  Active -> Ready
```

Request changes does not rewrite the completed run. The prior run remains
available with its summary, result, event log, PR information, and rejection
reason. A successful replacement run becomes the latest run for board
derivation.

## UI Design

The Ready task review panel replaces `Reject run` with `Request changes`.

- A textarea collects the required reason and shows the 2,000-character limit.
- An image picker supports file selection and drag-and-drop.
- Each selected image shows a thumbnail, filename, size, and remove action.
- Client validation enforces count, size, and supported types before submission.
- The action is disabled while invalid, empty, or submitting.
- Server validation errors stay in the panel and do not clear the user's input.
- On success, the task refreshes into Active and the live event surface follows
  the replacement run.
- The request-change reason and evidence thumbnails remain visible in later task
  detail views as historical feedback.

## HTTP Contract

Add:

```text
POST /api/runs/<current_run_id>/request-changes
Content-Type: multipart/form-data; boundary=...

reason=<UTF-8 text>
evidence=<binary image, repeated up to three times>
```

Successful response:

```json
{
  "source_run_id": "run_old",
  "run_id": "run_new",
  "story_id": "US-084",
  "status": "recovering",
  "feedback": {
    "reason_path": ".harness/runs/run_new/feedback/reason.md",
    "evidence_paths": [
      ".harness/runs/run_new/feedback/evidence-01.png"
    ]
  }
}
```

Expected errors:

- `400` for an empty/oversized reason, malformed multipart body, unsupported
  image signature, too many files, or files above 5 MB.
- `404` for an unknown run.
- `409` when the run is not the current Ready result, the story is no longer
  runnable, another run is active, or the task is Done.
- `500` only for unexpected filesystem or state-store failures.

## Bounded Request Reading

The current server reads one fixed 8 KB buffer and treats the request as UTF-8
text. Replace this with a bounded request reader that:

1. reads headers to the header terminator;
2. parses `Content-Length`;
3. refuses bodies above the configured request ceiling before allocation;
4. reads the exact body length as bytes;
5. preserves binary bodies for multipart parsing; and
6. retains the existing string path for JSON and static requests.

The multipart ceiling is 15 MB of image data plus bounded form/header overhead.
Connection chunking and unbounded streaming uploads remain out of scope for the
local v1 controller.

## Validation And File Safety

- Generate evidence filenames server-side; never use the browser filename as a
  filesystem path.
- Reject path separators, duplicate field abuse, unknown fields, and more than
  three evidence parts.
- Validate PNG, JPEG, and WebP using file signatures, not only MIME type or
  extension.
- Derive the stored extension from the validated signature.
- Write to a staging directory first and rename into the run feedback directory
  only after every part validates.
- Remove staged files when validation or run preparation fails.
- Do not serve arbitrary feedback paths through the static asset handler.

## Run Preparation And Atomicity

Introduce a feedback-aware replacement-run preparation path:

1. Confirm the source run is the current Ready result and no active run exists.
2. Validate and stage the reason and evidence files.
3. Prepare a new isolated run for the same story with optional feedback metadata
   included before `RUN_CONTRACT.json` is written.
4. Write the feedback directory in the new run workspace and durable root run
   artifact directory.
5. Mark the source run rejected with the submitted reason.
6. Spawn the replacement run.

If steps 1-4 fail, delete staging/prepared artifacts and leave the source run
unchanged. If the state transition fails after preparation, release the active
run lock and remove the incomplete replacement workspace before returning an
error.

## Run Contract And Agent Prompt

Add an optional contract field:

```json
{
  "request_changes": {
    "source_run_id": "run_old",
    "reason_path": ".harness/runs/run_new/feedback/reason.md",
    "evidence_paths": [
      ".harness/runs/run_new/feedback/evidence-01.png"
    ]
  }
}
```

The agent prompt must explicitly require reading the reason and inspecting every
evidence image before editing. Adapters that cannot inspect images still receive
the paths and must report the limitation rather than silently ignoring them.

## Artifact Retention And Review

Feedback lives under:

```text
.harness/runs/<new_run_id>/feedback/
  reason.md
  evidence-01.png
  evidence-02.jpg
```

It follows the existing run retention policy and is removed when that run is
compacted. The review API exposes feedback metadata and safe artifact presence,
while raw filesystem paths are not converted into public arbitrary-file routes.

## Observability

- Record source and replacement run IDs in state transitions and logs.
- Preserve the rejection reason on the source run.
- Include feedback paths in the replacement run contract, summary context, and
  review response.
- Return precise validation errors without echoing binary data or unsafe client
  filenames.

## Alternatives Considered

1. Multipart request with atomic validation and retry preparation. Selected
   because it keeps user intent and evidence in one bounded operation.
2. JSON base64 images. Rejected because of request expansion and unnecessary
   binary duplication.
3. Separate upload and retry endpoints. Rejected because orphan cleanup and
   two-step state are unnecessary for the local controller.

## Non-Goals

- Reopening Done tasks.
- Video, PDF, SVG, clipboard recording, or arbitrary file attachments.
- Cloud object storage or remote evidence hosting.
- Changing the story schema or adding a fifth board bucket.
- Editing an existing PR in place; replacement-run PR behavior continues to use
  the configured Symphony PR workflow.

## Validation Strategy

- Rust unit tests for bounded request reads, multipart parsing, signature
  validation, safe filenames, count/size limits, and cleanup.
- Rust integration tests for eligibility guards, old-run preservation,
  replacement-run creation, contract metadata, prompt content, and failure
  rollback.
- Playwright E2E for selecting images, client validation, Ready-to-Active
  transition, and feedback visibility.
- Web UI build, desktop smoke, full workspace tests, format, clippy, and diff
  hygiene.
