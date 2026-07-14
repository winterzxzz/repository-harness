# US-097 Branched Task Flow for PR and PR-less Runs

## Status

planned

## Lane

normal

## Product Contract

The active task lifecycle strip reflects the actual review path of the run.
A run that ships code through a pull request shows the PR branch
(`Start → Agent → Validation → Pull request → Review & merge → Sync → Done`).
A run with no pull request (`pr_status = not_applicable`, e.g. PR creation
disabled or no code changes) shows the local-review branch
(`Start → Agent → Validation → Review → Sync → Done`) with no PR step and no
"merge" wording. The human is never shown a lifecycle step that cannot happen
for the run they are reviewing.

## Relevant Product Docs

- `docs/SYMPHONY_QUICKSTART.md`
- `docs/stories/epics/E08-symphony-web-ui-controller/README.md`

## Acceptance Criteria

- `derive_task_flow` in `crates/harness-symphony/src/web.rs` selects the step
  list per run: the PR branch when a pull request exists or is expected, the
  local-review branch when `pr_status` is `not_applicable`.
- The local-review branch omits the `pr` step entirely and labels the review
  step "Review" (not "Review & merge").
- The PR branch keeps today's steps and labels unchanged.
- Step states (pending/current/complete/failed) remain correct on both
  branches, including recovery and request-changes states.
- The Web UI renders whichever step list the backend sends; the client no
  longer assumes the fixed 7-step list except as an empty-state placeholder.
- Existing Rust web tests cover both branch derivations; a Playwright test
  asserts the PR-less flow shows no "Pull request" step and shows "Review".

## Design Notes

- Commands: none.
- Queries: `GET /api/board` already returns `task_flow.steps`; only the
  derivation changes. No API shape change — the steps array is already
  client-driven.
- API: `TaskFlowStepId` gains no new ids; the `pr` step is simply absent on
  the local-review branch. Client `labels` map adds a context-aware label for
  `review` (backend may send the label, or client picks label from presence
  of the `pr` step).
- Tables: none.
- Domain rules: branch selection mirrors the sync gate — a run syncs either
  after PR merge (US-055 gate) or directly via `local_review_without_pr`;
  the flow strip must match whichever gate applies.
- UI surfaces: `active-task-flow.tsx` (render from server steps, fallback
  placeholder), `web.rs::derive_task_flow`.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-097 --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | Rust: `derive_task_flow` returns the PR branch and the local-review branch per `pr_status`; states correct on both. |
| Integration | Board response includes the branched step list for a PR-less completed run. |
| E2E | Playwright: PR-less run shows Review (no Pull request step); PR run unchanged. |
| Platform | n/a. |
| Release | Web UI dist rebuild included. |

## Harness Delta

None expected.

## Evidence

Add commands, reports, screenshots, or links after validation exists.
