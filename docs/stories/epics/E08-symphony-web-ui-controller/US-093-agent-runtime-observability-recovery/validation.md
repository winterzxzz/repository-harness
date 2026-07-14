# Validation

## Proof Strategy

Use deterministic fake Codex/OpenCode processes and temporary SQLite databases
to prove runtime semantics before browser coverage. Every behavior change uses
red-green-refactor. Release proof covers Rust, Web UI, browser, and platform
process behavior.

## Test Plan

| Layer | Cases |
| --- | --- |
| Unit | State migration/control, uncapped Codex, cancellation, normalized event ordering/retention, OpenCode output mapping, stage derivation |
| Integration | Start ownership, cursor event API, cancel API, startup orphan reconciliation, validation/PR/sync stage transitions |
| E2E | Codex/OpenCode live events, Cancel confirmation/result, lifecycle progression, retry after interruption |
| Platform | Unix process-group descendant termination and non-Unix compilation behavior; Electron smoke |
| Performance | Cursor polling transfers only new events; retained artifact stays bounded |
| Logs/Audit | Raw adapter artifacts remain; normalized terminal/cancel/interruption events are retained |

## Fixtures

- Fake Codex app-server capable of staying in progress past an injected former
  deadline, returning terminal state, stalling, and spawning a descendant.
- Fake OpenCode command that emits interleaved stdout/stderr and waits for
  cancellation.
- Temporary state database containing legacy and stale active run rows.
- Playwright board fixtures with incremental normalized event responses.

## Commands

```text
cargo fmt --check
cargo test -p harness-symphony
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix crates/harness-symphony/web-ui run build
npm --prefix crates/harness-symphony/web-ui run e2e
git diff --check
```

## Acceptance Evidence

Record exact passing counts, platform smoke results, and any unavailable proof
before changing the durable story status to implemented.

