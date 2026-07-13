# US-092 Symphony Runtime Cleanup

## Status

planned

## Lane

normal

## Product Contract

Fresh Harness installations must ignore local Symphony runtime state, and
Symphony must safely reclaim terminal worktrees without deleting permanent
changesets or branches that may contain unmerged code.

## Relevant Product Docs

- `docs/SYMPHONY_SCOPE.md`
- `docs/superpowers/specs/2026-07-13-symphony-runtime-cleanup-design.md`

## Acceptance Criteria

- Bash and PowerShell installers idempotently add `.symphony/`, `.worktrees/`,
  and local `.harness/` ignore rules while keeping changesets visible to Git.
- A successfully synced Done run removes its registered worktree when cleanup
  is enabled but preserves its branch, changeset, and run record.
- Failed, interrupted, and orphan worktrees are eligible after seven days by
  default; prepared and running worktrees are never eligible.
- `harness-symphony runs cleanup [--dry-run]` reports candidates, reasons,
  outcomes, and best-effort reclaimed bytes and is safe to rerun.
- Startup cleanup and post-sync cleanup are best effort and never reverse a
  successful run or sync outcome.
- Cleanup rejects paths outside the configured worktree root and cannot follow
  symlinks to delete external content.
- Automatic run-artifact compaction retains the newest configured terminal-run
  evidence, never removes active-run evidence, and never removes
  `.harness/changesets/`.

## Design Notes

- Commands: `harness-symphony runs cleanup [--dry-run]`.
- Queries: read local run state and `git worktree list`; no Harness DB schema
  query is required.
- API: no Web API change in this slice.
- Tables: no Harness DB schema change; local Symphony state remains the runtime
  source of truth.
- Domain rules: Done worktrees clean immediately after sync; failed,
  interrupted, and orphan worktrees use a configurable seven-day retention;
  branches and changesets are preserved.
- UI surfaces: existing run detail should distinguish a cleaned worktree path
  from a missing or corrupt active worktree.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-092 --unit 1 --integration 1 --e2e 0 --platform 1`.

| Layer | Expected proof |
| --- | --- |
| Unit | Cleanup eligibility, retention, path containment, active-run protection, and idempotence tests. |
| Integration | Temporary-repository worktree removal, branch preservation, dry-run, orphan, and partial-failure tests. |
| E2E | n/a. |
| Platform | Bash and PowerShell installer validation plus fresh-install Git-ignore smoke. |
| Release | `cargo test --workspace`; `cargo fmt --check`; `cargo clippy --workspace -- -D warnings`; `scripts/validate-install-payload.sh`; `git diff --check`. |

## Harness Delta

Turns existing passive cleanup configuration into an enforced fresh-install
and runtime lifecycle contract.

## Evidence

Add implementation and validation evidence after execution.
