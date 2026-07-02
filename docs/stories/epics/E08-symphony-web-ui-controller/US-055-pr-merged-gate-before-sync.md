# US-055 PR Merged Gate Before Sync

## Status

implemented

## Lane

high-risk

## Product Contract

The Symphony Web UI must require an accepted pull request before a user can
approve sync, so local durable state only moves to Done after the reviewed
changeset has been accepted.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`

## Acceptance Criteria

- Run state stores a pull-request status independently from run execution
  status.
- The Web API exposes a local MVP endpoint for marking a reviewed PR as merged.
- `POST /api/runs/<run-id>/sync` refuses to apply until the run PR status is
  `merged`.
- The browser review panel exposes Mark Merged and only enables Approve Sync
  after merge status is recorded.
- Board Done derivation still depends on a completed run whose accepted
  changeset has been synced.

## Design Notes

- Commands: `harness-symphony web`; existing `harness-symphony sync`.
- Queries: `POST /api/runs/<run-id>/pr-merged`,
  `POST /api/runs/<run-id>/sync`.
- API: run review and sync responses carry PR and sync state.
- Tables: reuses `.symphony/state.db` `run_state.pr_status`.
- Domain rules: PR merge status is manually recorded in the local MVP; it is a
  gate before sync, not proof that changes were applied.
- UI surfaces: review panel PR controls and Approve Sync action.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-055 --unit 1 --integration 1 --e2e 0 --platform 1`.

| Layer | Expected proof |
| --- | --- |
| Unit | State-store tests cover PR status transitions. |
| Integration | Web route tests prove sync is blocked until PR status is merged. |
| E2E | Browser flow can be expanded after disposable mutation fixtures exist. |
| Platform | Web UI build proves the review controls compile against the API contract. |
| Release | Not required. |

## Harness Delta

No process change. The story clarified that PR acceptance and changeset sync are
separate local state transitions.

## Evidence

- `cargo test -p harness-symphony pr_merge` passed.
- `cargo test -p harness-symphony web` passed.
- `npm --prefix crates/harness-symphony/web-ui run build` passed.
