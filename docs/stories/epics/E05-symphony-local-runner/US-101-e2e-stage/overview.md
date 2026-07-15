# Overview

## Current Behavior

A Symphony run flows `start → agent → validation → review → sync → done`. The
agent runs the project's tests itself during the `agent` stage and reports
evidence in `RESULT.json`. The `validation` stage only checks artifacts
(SUMMARY.md, RESULT.json schema, changeset digest) in milliseconds; Symphony
never executes any test independently, so a wrong `pass` claim by the agent is
not caught.

## Target Behavior

A new durable stage `e2e` runs between `agent` and `validation`:
`start → agent → e2e → validation → review → sync → done`. After the agent
finishes, Symphony itself executes the story's declared E2E command inside the
run worktree, streams its output into `RUN_EVENTS.jsonl` (stage `e2e`) so the
Web UI console shows it live, and fails the run when the command exits
non-zero. Stories without an E2E command skip the stage cleanly with an
explanatory event. The Web UI task flow diagram shows the `e2e` node between
`agent` and `validation`.

## Affected Users

- Operator reviewing runs on the Symphony Web UI board.
- Agents (managed adapters and external subagents) whose runs gain an
  independent verification step.

## Affected Product Docs

- `docs/SYMPHONY_QUICKSTART.md` (run lifecycle, story authoring)
- `docs/TEST_MATRIX.md` (E2E proof layer)

## Non-Goals

- No review approval gate (that is US-102).
- No re-run of unit/integration tests by Symphony; only the single declared
  E2E command runs independently.
- No per-project default E2E command in `symphony.yml`; the command is
  declared per story.
