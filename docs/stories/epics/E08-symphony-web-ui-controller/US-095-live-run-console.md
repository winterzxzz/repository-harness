# US-095 Live Run Console in Web UI

## Status

planned

## Lane

normal

## Product Contract

While a Symphony run is active (or after it finishes), the run detail view in
the Web UI offers a read-only "console" transcript of the executing agent —
the same experience as viewing a subagent transcript in Claude Code or Codex.
The human sees, in order and near real time: each command the agent starts,
its streamed output, agent milestone messages, and failures. The console is
observational only; it never accepts input into the run worktree.

## Relevant Product Docs

- `docs/SYMPHONY_QUICKSTART.md`
- `docs/stories/epics/E08-symphony-web-ui-controller/README.md`

## Acceptance Criteria

- The run detail view (`detail.tsx`) exposes a console pane with full
  scrollback for the selected run, not just the last 12 summarized lines the
  current `EventLog` shows.
- The pane renders, in event order: `commandExecution` starts (the command
  line), streamed `outputDelta` text grouped under its command, agent
  milestone messages (`agentMessage` completions), and non-zero exit codes
  visibly marked as failures.
- Reasoning-delta and other high-volume non-actionable event types are
  filtered out client-side so the transcript stays readable.
- The pane follows live output (auto-scroll) while the run is active and
  stops following when the user scrolls up; a control returns to tail.
- ANSI escape sequences in command output are stripped or rendered, never
  shown as raw escape bytes.
- The console is read-only: no input field, no endpoint that forwards
  keystrokes or commands into the run worktree.
- Works against the existing polling endpoint
  `GET /api/runs/<id>/events?after=N`; no new server capability is required.
  Client-side memory is bounded (cap retained transcript length) so a
  long run cannot grow the tab without limit.

## Design Notes

- Commands: none (read-only surface).
- Queries: existing `GET /api/runs/<id>/events?after=N` incremental polling,
  already consumed by `detail.tsx`.
- API: no server changes expected; `web.rs` already paginates
  `RUN_EVENTS.jsonl` via `events_response`.
- Tables: none.
- Domain rules: interactive terminal access to the run worktree is explicitly
  out of scope — it would break run isolation and changeset validation
  (see US-094 external executor design).
- UI surfaces: new `RunConsole` component in
  `crates/harness-symphony/web-ui/src/features/symphony/`, mounted in the run
  detail view alongside or replacing the summary `EventLog`; event parsing
  shares the envelope decoding already present in `detail.tsx`
  (`kind`/`message` JSON with app-server `method`/`params.item` payloads).

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-095 --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | Event-to-transcript formatting: command start, output grouping, failure marking, reasoning-noise filtering, ANSI stripping, buffer cap. |
| Integration | Console consumes a recorded `RUN_EVENTS.jsonl` fixture through the polling client and renders the expected transcript. |
| E2E | Open run detail for an active run, observe live transcript following output; scroll-up pauses follow. |
| Platform | n/a (browser-only surface). |
| Release | Web UI dist rebuild included in packaged kit. |

## Harness Delta

None expected; note friction here if event volume forces a server-side filter
parameter on the events endpoint.

## Evidence

Add commands, reports, screenshots, or links after validation exists.
