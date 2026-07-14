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

- `cargo fmt --check`, `cargo test --workspace`, and
  `cargo clippy --workspace -- -D warnings` passed. The workspace run included
  253 Symphony tests plus 63 Harness CLI/bench tests and doc tests.
- The Web UI production build passed, all 41 Playwright tests passed, and the
  Electron desktop smoke passed.
- `scripts/validate-install-payload.sh` passed for the fresh installer,
  merge/update paths, agent-shim refresh, and shared Bash/PowerShell payload.
- Temporary lifecycle run `run_1784006084764676000_80083_0` completed through
  prepare, external start, heartbeat milestone, shared artifact validation,
  and canonical `completed` state with executor `claude-subagent`.
- A separate one-second-TTL fixture proved stale lock release and late
  completion while newer run `run_1784006102600026000_83724_0` remained the
  active prepared run.
- Review regressions cover strict semantic changeset validation, digest-error
  lock release, changed-database rejection, heartbeat/expiry concurrency,
  request/prepare/cleanup reconciliation, normalized external review events,
  and late completion while a newer run owns the active lock.
- `git diff --check` passed.
