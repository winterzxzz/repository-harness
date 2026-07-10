# Design

## Domain Model

`RequestChangesFeedback` contains:

- `source_run_id`: the Ready run being rejected.
- `reason`: trimmed UTF-8 text from 1 to 2,000 characters.
- `evidence`: zero to three validated images.

Each validated image contains a server-generated filename, detected image kind,
byte length, and staged bytes/path. The original browser filename is display
metadata only and never controls filesystem placement.

The source run and replacement run are distinct immutable attempts. The source
run becomes rejected only after the replacement run and feedback artifacts are
prepared successfully.

## Application Flow

1. Parse a bounded multipart request.
2. Validate the reason, part count, image sizes, and image signatures.
3. Load the source run and derive the current board item.
4. Require current Ready eligibility and an available active-run lock.
5. Stage sanitized feedback.
6. Prepare a replacement run for the same story with feedback contract data.
7. Persist feedback into the worktree and durable root run artifact directory.
8. Mark the source run rejected with the reason.
9. Spawn the replacement agent process.
10. Return the replacement run and feedback metadata; refresh the board.

Failure before source-run rejection leaves the source run unchanged. Failure
after replacement preparation must release its active lock and remove incomplete
workspace/artifact state.

## Interface Contract

Route:

```text
POST /api/runs/<run_id>/request-changes
Content-Type: multipart/form-data
```

Fields:

- `reason`: exactly one UTF-8 text part.
- `evidence`: zero to three binary image parts.

Response codes:

- `202`: replacement run prepared and spawned.
- `400`: malformed or invalid feedback.
- `404`: source run not found.
- `409`: source is not current Ready work, story cannot run, task is Done, or
  another run is active.
- `500`: unexpected local I/O or state failure.

The review response exposes feedback reason/evidence metadata for historical
display without adding arbitrary filesystem download endpoints.

## Data Model

No Harness SQLite schema migration is required. Existing run state stores the
source rejection status and replacement run record.

Filesystem artifacts:

```text
.harness/runs/<replacement_run_id>/feedback/reason.md
.harness/runs/<replacement_run_id>/feedback/evidence-01.<ext>
```

The replacement `RUN_CONTRACT.json` gains an optional `request_changes` object.
Run compaction removes feedback with the replacement run directory; changesets
and story history remain unaffected.

## UI / Platform Impact

- Replace the Ready review panel's reject control with a request-changes form.
- Add textarea validation, drag/drop file selection, thumbnails, file removal,
  and upload error display.
- On success, refresh into Active and follow the replacement run's event log.
- Keep the control unavailable for Done and non-Ready tasks.
- Desktop and browser builds share the same local API and artifact behavior.

## Observability

- Log source and replacement run IDs without logging binary bodies.
- Preserve the submitted reason in source-run next action and replacement
  feedback artifacts.
- Add feedback paths to the replacement contract and prompt.
- Record validation/cleanup failures with actionable messages.

## Alternatives Considered

1. One bounded multipart request. Selected for atomic user intent and evidence.
2. JSON base64. Rejected because of memory and request-size expansion.
3. Separate upload endpoint. Rejected because of orphaned-upload complexity.
4. New task or new board bucket. Rejected because this is another attempt at the
   same story and the existing Active bucket already represents rerun work.
