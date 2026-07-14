# US-098 Forked Task Flow Diagram Showing Both Review Paths

## Status

planned

## Lane

normal

## Product Contract

The active task lifecycle strip always draws the full forked shape of the
lifecycle: a shared head (`Start → Agent → Validation`), a fork into two
parallel lanes — the PR lane (`Pull request → Review & merge`) and the
local-review lane (`Review`) — and a shared tail (`Sync → Done`). The lane
the current run actually takes carries the live step states; the other lane
is rendered dimmed as a not-taken path. While the run has not yet decided its
path (`pr_status` still `missing`), both lanes render neutral/pending and
only the shared head/tail carry states. The human always sees where a task
could go and where it actually went.

## Relevant Product Docs

- `docs/SYMPHONY_QUICKSTART.md`
- `docs/stories/epics/E08-symphony-web-ui-controller/README.md`

## Prerequisite

US-097 (branched step derivation) must land first; this story builds the
two-lane visualization on top of its canonical PR / local-review branches.

## Acceptance Criteria

- The lifecycle strip renders the forked diagram for every task with a flow:
  shared head, two labelled lanes between fork and join points, shared tail.
- The taken lane shows real step states (pending/current/complete/failed)
  from the server-provided steps; the not-taken lane renders dimmed with a
  distinct "not taken" treatment (no state icons that could read as pending
  work).
- While the path is undecided (server sends the PR branch by default during
  an active run and `pr_status` is `missing`), both lanes render as neutral
  candidates and neither is dimmed; shared head/tail still show live states.
- Once `pr_status` becomes `not_applicable`, states map onto the
  local-review lane and the PR lane dims; once a PR exists (`created`,
  `merged`, `failed`), states map onto the PR lane and the local-review lane
  dims.
- The fork renders within the existing horizontal-scroll container and stays
  legible at the board's minimum supported width; accessibility: the list
  semantics announce lane membership and taken/not-taken status.
- No API change: the client derives the fork from the existing
  `task_flow.steps` canonical branch plus `pr_status` already present in the
  board/review payloads (extend the board item payload only if `pr_status`
  is not already available to the flow component; if extension is needed it
  is additive).
- Playwright coverage: undecided run shows both lanes neutral; PR run shows
  local lane dimmed; PR-less completed run shows PR lane dimmed.

## Design Notes

- Commands: none.
- Queries: existing `GET /api/board` `task_flow`; possibly additive
  `pr_status` on the task_flow payload if the component cannot see it today.
- API: no breaking change; US-097 branch parsing stays.
- Tables: none.
- Domain rules: lane selection mirrors US-097 branch selection exactly; the
  diagram must never show the taken lane contradicting the sync gate.
- UI surfaces: `active-task-flow.tsx` rewritten from a flat `<ol>` into a
  fork layout (shared segments + two lanes); `api.ts` unchanged or additive.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-098 --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | Lane-mapping helper: steps → taken lane per pr_status, undecided handling. |
| Integration | Rust web test asserts task_flow payload carries what the fork needs (pr_status or equivalent). |
| E2E | Playwright: three states — undecided (both neutral), PR taken (local dimmed), local taken (PR dimmed). |
| Platform | n/a. |
| Release | Web UI dist rebuild included. |

## Harness Delta

None expected.

## Evidence

Add commands, reports, screenshots, or links after validation exists.
