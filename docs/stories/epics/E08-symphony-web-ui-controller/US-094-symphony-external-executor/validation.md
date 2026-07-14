# Validation

## Proof Strategy

Prove external execution through the same state, validation, event, board, and
installer surfaces used by managed runs. Exercise races with deterministic
clocks and temporary repositories before one real Claude subagent run.

## Test Plan

| Layer | Cases |
| --- | --- |
| Unit | Migration default; start/heartbeat/complete guards; TTL boundaries; heartbeat-expiry transaction race; canonical outcomes; logical DB digest |
| Integration | Full prepare/start/heartbeat/complete; stale lock release; late completion during newer active run; invalid artifacts; changed DB without changeset |
| E2E | Executor badge, milestone event, stale Needs Attention state, live external run surviving Web restart |
| Platform | Browser build/E2E, Electron smoke, Bash and PowerShell fresh-install payload validation |
| Performance | Reconciliation and logical digest remain bounded for the local Harness database fixture |
| Logs/Audit | Normalized start, changed milestone, expiry, validation, and terminal events without heartbeat spam |

## Fixtures

- Temporary git repository with one runnable story.
- Temporary state DB containing managed, external-running, external-stale, and
  migrated legacy rows.
- Copied Harness databases with unchanged data, semantic CLI writes, and a
  mutation lacking a changeset.
- Deterministic clock and concurrent heartbeat/expiry barriers.

## Commands

```text
cargo test -p harness-symphony
cargo test --workspace
cargo fmt --check
cargo clippy --workspace -- -D warnings
npm --prefix crates/harness-symphony/web-ui run build
npm --prefix crates/harness-symphony/web-ui run e2e
npm --prefix crates/harness-symphony/web-ui run desktop:smoke
scripts/validate-install-payload.sh
git diff --check
scripts/bin/harness-cli story verify US-094
```

## Acceptance Evidence

Pending implementation. Final evidence must include one monitored Claude
main-agent/subagent run from prepare through validated completion.
