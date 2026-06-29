# US-060 Human-Readable Chat Logs

## Status

implemented

## Lane

normal

## Product Contract

The Web UI must present Codex run logs as a readable conversation and progress
timeline instead of exposing raw JSON-RPC event payloads as the primary view.
Raw event artifacts must remain accessible for technical review and debugging.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`

## Acceptance Criteria

- The task detail and review surfaces show run communication in human-readable
  entries with clear speaker/source labels, timestamps when available, and
  concise message text.
- Incremental agent message deltas are combined into coherent messages instead
  of appearing as fragmented event rows.
- Non-chat lifecycle events such as `turn/started`, `turn/diff/updated`, and
  `turn/completed` are summarized as progress milestones.
- Empty, malformed, or unsupported event payloads degrade to a useful fallback
  label without breaking the UI.
- The raw `APP_SERVER_EVENTS.jsonl` artifact path or raw event view remains
  reachable from the review surface for maintainers.
- The display works in the browser UI and the Electron desktop shell because it
  reuses the existing React surface and local `/api/*` contract.

## Design Notes

- Commands: `harness-symphony web`; existing desktop build uses the same Web UI
  assets.
- Queries: reuse `GET /api/runs/<run-id>/events` and
  `GET /api/runs/<run-id>/review`.
- API: either add presentation-safe fields to event/review responses or keep
  the API raw and map events in the React client; preserve backwards-compatible
  access to raw event data.
- Tables: none.
- Domain rules: UI summaries are presentation-only and must not become a second
  source of truth for Symphony run state.
- UI surfaces: active run logs in the task detail popup and review logs for
  Review, Needs Attention, and Done tasks.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-060 --unit 1 --integration 1 --e2e 1 --platform 1`.

| Layer | Expected proof |
| --- | --- |
| Unit | Formatter/parser tests cover agent message deltas, lifecycle events, malformed payloads, and raw fallback behavior. |
| Integration | Web route or fixture test proves review/event payloads render into readable log entries without losing raw artifact access. |
| E2E | Playwright verifies the Web UI shows readable log text for a mocked or fixture-backed run. |
| Platform | Electron smoke or existing desktop build verifies the shared UI compiles/runs with the log changes. |
| Release | Not required. |

## Harness Delta

No process change.

## Evidence

- Added a React-side Codex run log formatter that combines
  `item/agentMessage/delta` payloads, summarizes lifecycle events, and keeps
  unsupported payloads readable without changing the raw event API.
- Updated the task detail/review log panel to show human-readable run
  communication while preserving `APP_SERVER_EVENTS.jsonl` as a visible raw
  artifact.
- Validation passed:
  `npm --prefix crates/harness-symphony/web-ui run build`;
  `npm --prefix crates/harness-symphony/web-ui run e2e`;
  `cargo test -p harness-symphony web`;
  `npm --prefix crates/harness-symphony/web-ui run desktop:smoke`;
  `git diff --check`.
