# 0011 Independent E2E Stage Between Agent And Validation

Date: 2026-07-15

## Status

Accepted

## Context

A Symphony run flowed `start → agent → validation → review → sync → done`. The
agent ran the project's tests itself and reported evidence in `RESULT.json`;
the `validation` stage only checked artifacts, so a wrong `pass` claim by the
agent was never caught by Symphony itself (US-101).

## Decision

Add a durable `e2e` stage between `agent` and `validation` in every run:

- `story.e2e_command TEXT` (migration `scripts/schema/009-story-e2e.sql`)
  declares the story's exact E2E command; it is set with
  `harness-cli story add|update --e2e-command "<command>"`. The flag is named
  `--e2e-command` (not the designed `--e2e`) because `story update --e2e 0|1`
  already records the E2E proof column.
- Prepare embeds the command as an additive optional `e2e_command` field in
  `RUN_CONTRACT.json` (contract version unchanged), so managed and external
  runs finalize identically.
- `finalize_prepared_run` executes the command in the run worktree via the
  platform shell (`sh -c`, PowerShell on Windows), streams stdout/stderr into
  `RUN_EVENTS.jsonl` as `output` events with stage `e2e`, and bounds it with
  `e2e.timeout_minutes` (default 15, zero rejected). Exit 0 records
  `e2e passed`; non-zero exit or timeout fails the run with next action
  `inspect e2e failure`. Stories without a command skip cleanly with an
  `e2e skipped` lifecycle event.
- Web flow stage lists and the Web UI task-flow diagram show `e2e` between
  `agent` and `validation`.

## Alternatives Considered

1. Run the E2E command inside the existing `validation` stage — hides the step
   the operator asked to see; rejected.
2. Project-wide E2E command in `symphony.yml` — cannot vary per story;
   rejected by the operator.
3. Keep trusting agent-reported E2E evidence in `RESULT.json` — keeps the
   blind spot the stage exists to remove; rejected.

## Consequences

Positive:

- A wrong `pass` claim by an agent is caught before validation/review.
- Operators watch E2E output live in the Web UI console.

Tradeoffs:

- Runs with a declared command take as long as the E2E suite on every run.
- Only the single declared E2E command runs independently; unit/integration
  proofs remain agent-reported.

## Follow-Up

- Retry policy for flaky E2E commands is out of scope until needed.
