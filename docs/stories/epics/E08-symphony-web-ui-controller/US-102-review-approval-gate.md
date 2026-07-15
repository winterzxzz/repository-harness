# US-102 Review Approval Gate Before Sync

## Status

implemented

## Lane

normal

## Product Contract

A completed run cannot be synced into the root `harness.db` until a human has
explicitly approved it. When pull-request creation is disabled, the Web UI
review screen offers an Approve action; sync (Web endpoint and CLI) refuses
runs that are neither approved nor merged via PR. The board keeps the task in
the review column until the operator decides.

## Relevant Product Docs

- `docs/SYMPHONY_QUICKSTART.md` (review and sync lifecycle)

## Acceptance Criteria

- Run state records an explicit approval (`reviewed_at` timestamp plus
  reviewer note column on `run_state`, auto-migrated by `ensure_column` like
  `execution_mode` was).
- New Web endpoint `POST /api/runs/<id>/approve` sets the approval; only
  valid for `completed` runs, idempotent.
- Web sync endpoint: `local_review_without_pr` additionally requires the
  approval; unapproved completed runs get a 409 explaining "approve the run
  before sync".
- CLI `harness-symphony sync` skips changesets belonging to completed,
  PR-less, unapproved runs and reports them as blocked instead of applying
  them; `sync` for merged-PR runs is unchanged.
- Board: completed unapproved runs stay in the review column with next action
  "approve or request changes"; they no longer jump straight to done.
- Request-changes flow is unchanged and remains the rejection path.
- Existing terminal/stale reconciliation, retention, and cleanup behavior is
  unchanged.

## Design Notes

- Commands: `POST /api/runs/<id>/approve`; optional CLI
  `harness-symphony runs approve <run_id>` for terminal-only workflows.
- Queries: sync eligibility checks approval; board next-action derivation.
- Tables: `run_state` + `reviewed_at INTEGER NULL` (ensure_column migration;
  state.db is runtime-local, no scripts/schema change).
- Domain rules: approval only on `completed`; PR-merged runs bypass approval
  (the PR review was the approval).
- UI surfaces: review screen Approve button; board review column badge.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-102 --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | approval eligibility rules; sync-blocking predicate |
| Integration | approve endpoint happy/invalid paths; web sync 409 for unapproved; CLI sync skips and reports blocked changesets; ensure_column migration on existing state.db |
| E2E | binary smoke: completed run blocked from sync until `runs approve`, then syncs |
| Platform | n/a |
| Release | cargo fmt/clippy/test -p harness-symphony |

## Harness Delta

Closes the gap found on 2026-07-15: review was a screen, not a gate — every
completed PR-less run was immediately sync-eligible and agents ran `sync`
before the operator ever saw the review column.

## Evidence

- `cargo test -p harness-symphony`: 274 passed.
- `cargo clippy -p harness-symphony -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `npm --prefix crates/harness-symphony/web-ui test`: 12 passed.
- `npm --prefix crates/harness-symphony/web-ui run build`: passed.
- `npm --prefix crates/harness-symphony/web-ui run e2e`: 47 passed.
- `scripts/bin/harness-cli story verify US-102`: passed in the run-scoped DB.
