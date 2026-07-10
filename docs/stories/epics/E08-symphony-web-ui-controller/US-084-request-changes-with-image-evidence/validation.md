# Validation

## Proof Strategy

Prove both the accepted path and every guard around untrusted uploads and state
transition. The task is complete only when invalid feedback cannot mutate the
source run or start an agent, valid feedback reaches the replacement contract
and prompt, and the browser visibly transitions Ready to Active.

## Test Plan

| Layer | Cases |
| --- | --- |
| Unit | Header/body boundary reads; Content-Length ceiling; multipart fields; reason limits; file count/size; PNG/JPEG/WebP signatures; safe generated names; cleanup helpers. |
| Integration | Current Ready eligibility; Done/non-current/active conflicts; replacement run creation; source rejection ordering; feedback artifacts in root/worktree; contract and prompt paths; rollback on preparation/state failure. |
| E2E | Required reason; image preview/removal; invalid type/size/count; successful Request changes; Ready-to-Active transition; prior feedback remains visible. |
| Platform | Web UI build and Electron desktop smoke with the shared upload flow. |
| Performance | Maximum accepted request completes without unbounded allocation; over-limit Content-Length is refused before body allocation. |
| Logs/Audit | Source/replacement IDs and reason paths are visible; binary data and unsafe filenames are not logged. |

## Fixtures

- Minimal valid PNG, JPEG, and WebP byte fixtures.
- Invalid signature with a misleading `.png` filename.
- One file above 5 MB and a four-file request.
- Ready source run, Done source run, stale source run, and active-run conflict.
- Replacement preparation failure fixture for rollback proof.

## Commands

```text
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony request_changes -- --nocapture
npm --prefix crates/harness-symphony/web-ui run build
npm --prefix crates/harness-symphony/web-ui run e2e
npm --prefix crates/harness-symphony/web-ui run desktop:smoke
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test --workspace
cargo fmt --check
cargo clippy --workspace -- -D warnings
git diff --check
scripts/bin/harness-cli story verify US-084
```

## Acceptance Evidence

- Focused Rust proof: 25 `request_changes` tests passed, covering bounded HTTP
  reads, multipart limits/signatures, atomic state replacement, filesystem
  rollback, feedback contracts, agent prompts, endpoint conflicts, review
  metadata, exact binary serving, and traversal rejection.
- Browser proof: production TypeScript/Vite build passed and all 35 Playwright
  tests passed, including multipart submission, preview/removal, type/size/count
  guards, historical evidence rendering, Ready-to-Active transition, and the
  Done-task guard.
- Platform proof: Electron desktop smoke passed at a local ephemeral backend.
  In the linked worktree the ignored durable `harness.db` was provided through
  a temporary symlink to the checkout-root database, then removed after proof.
- Workspace proof: 17 `harness-bench` unit tests, 3 score CLI integration tests,
  41 `harness-cli` tests, and 149 `harness-symphony` tests passed with zero
  failures; doc tests also passed.
- Static proof: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`,
  and `git diff --check` passed.
- Durable proof: `harness-cli story verify US-084` passed after executing the
  complete configured verification command.
- Decision proof: durable decision `0008` remains `accepted`. The current record
  has no `verify_command`, so `decision verify 0008` reports that configuration
  gap; `query decisions` confirms the accepted decision record without mutating
  it.
