# Symphony Runtime Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Harness Symphony agent execution bounded, crash-recoverable, validation-truthful, and safe for unattended local use.

**Architecture:** Introduce focused process supervision helpers inside the Symphony crate, extend the existing SQLite state schema for leases and delayed retries, and tighten existing run/PR gates. Preserve the current Harness-local single-agent model and existing public CLI shape.

**Tech Stack:** Rust, rusqlite, Codex app-server JSON-RPC, Git worktrees, crate-local unit/integration tests.

---

### Task 1: Supervise Agent Processes And Bound Output

**Files:**
- Modify: `crates/harness-symphony/src/agent.rs`
- Modify: `crates/harness-symphony/src/config.rs`
- Test: `crates/harness-symphony/src/agent.rs`

- [ ] **Step 1: Write failing tests for wall timeout, stderr draining, and bounded adapter output**

Add tests using executable shell fixtures that sleep beyond a short test deadline,
write more than a pipe buffer to stderr, and emit output beyond the configured
artifact cap. Assert timeout returns `AgentError::Timeout`, stderr-heavy Codex
finishes without deadlock, and logs end with a truncation marker.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p harness-symphony agent::tests -- --nocapture`

Expected: new tests fail because timeout, concurrent stderr drain, and bounded
streaming do not exist.

- [ ] **Step 3: Implement the minimal supervisor**

Add `AgentError::Timeout`, reject `timeout_minutes == 0`, replace
`Command::output()` with piped streaming, drain both pipes concurrently, write
through a capped artifact writer, and check a monotonic deadline in every
adapter. On Unix, start agents in a process group and terminate the group on
timeout/error; keep platform-specific termination behind helper functions.

- [ ] **Step 4: Verify GREEN and regression coverage**

Run: `cargo test -p harness-symphony agent::tests config::tests -- --nocapture`

Expected: all focused tests pass with no hanging fixture processes.

### Task 2: Enforce Validation Truth At Run And PR Boundaries

**Files:**
- Modify: `crates/harness-symphony/src/run.rs`
- Modify: `crates/harness-symphony/src/pr.rs`
- Test: `crates/harness-symphony/src/run.rs`
- Test: `crates/harness-symphony/src/pr.rs`

- [ ] **Step 1: Write failing result-policy tests**

Add tests proving `outcome: completed` is rejected when any command is `fail` or
`unavailable`, and that PR planning re-checks the promoted `RESULT.json` rather
than trusting a stored `completed` status.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p harness-symphony run::tests pr::tests -- --nocapture`

Expected: completed-with-failed-proof and PR-planning tests fail.

- [ ] **Step 3: Add a shared completion policy**

Expose a crate-visible result validation helper. Require every command to be
`pass` for `completed`; retain schema-valid `fail` and `unavailable` evidence for
non-completed outcomes. Reparse `RESULT.json` in `plan_pr` and reject a ready PR
when completion proof is not passing.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p harness-symphony run::tests pr::tests -- --nocapture`

Expected: all focused tests pass.

### Task 3: Recover Orphan Queue Work And Add Retry Backoff

**Files:**
- Modify: `crates/harness-symphony/src/state.rs`
- Modify: `crates/harness-symphony/src/auto.rs`
- Modify: `crates/harness-symphony/src/config.rs`
- Test: `crates/harness-symphony/src/state.rs`
- Test: `crates/harness-symphony/src/auto.rs`

- [ ] **Step 1: Write failing state-machine tests**

Add deterministic tests that simulate restart with a `running` queue row and an
expired lease, verify it becomes queued/interrupted, and verify failed attempts
cannot be selected until their exponential `next_attempt_at` deadline.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p harness-symphony state::tests auto::tests -- --nocapture`

Expected: orphan recovery and delayed selection tests fail.

- [ ] **Step 3: Migrate state and implement atomic reconciliation**

Add nullable owner token, heartbeat, lease expiry, and next-attempt columns with
idempotent `ALTER TABLE` migration helpers. Claim queue work transactionally,
refresh leases around active execution, reconcile expired owners at auto startup,
and compute capped 10s/20s/40s/... retry delays using an injected timestamp.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p harness-symphony state::tests auto::tests -- --nocapture`

Expected: restart and retry timing tests pass without sleeping.

### Task 4: Roll Back Failed Preparation And Fail Closed On Stale Bases

**Files:**
- Modify: `crates/harness-symphony/src/run.rs`
- Modify: `crates/harness-symphony/src/auto.rs`
- Modify: `crates/harness-symphony/src/config.rs`
- Test: `crates/harness-symphony/src/run.rs`
- Test: `crates/harness-symphony/src/auto.rs`

- [ ] **Step 1: Write failing cleanup and freshness tests**

Inject a failure after worktree creation and assert no branch/worktree/run
directory remains. Add an auto test proving refresh failure prevents polling and
execution unless `allow_stale_base` is enabled.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p harness-symphony run::tests auto::tests -- --nocapture`

Expected: leaked preparation resources and stale-base dispatch tests fail.

- [ ] **Step 3: Share rollback cleanup and add stale-base policy**

Wrap ordinary preparation in the same cleanup path used by replacement runs.
Add `auto.allow_stale_base` defaulting to false. Return a typed auto error on
refresh failure when false; when true, continue and record the current base SHA
in diagnostic output/run metadata where the existing contract supports it.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p harness-symphony run::tests auto::tests -- --nocapture`

Expected: all cleanup and freshness tests pass.

### Task 5: Verify Web Identity And Document The Harness-Local Profile

**Files:**
- Modify: `crates/harness-symphony/src/web.rs`
- Modify: `docs/SYMPHONY_QUICKSTART.md`
- Modify: `docs/SYMPHONY_SCOPE.md`
- Modify: `README.md`
- Test: `crates/harness-symphony/src/web.rs`

- [ ] **Step 1: Write failing health identity tests**

Add listener fixtures for a foreign TCP service, a Symphony health response for
another repository, and a matching response. Assert only the matching service is
reported as `AlreadyRunning`.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p harness-symphony web::tests -- --nocapture`

Expected: TCP-only reuse behavior fails the new identity tests.

- [ ] **Step 3: Implement HTTP identity and update documentation**

Return JSON from `/health` with `service`, crate `version`, and a deterministic
repository-root identity. Make `ensure_web_server` perform a bounded HTTP request
and match identity. Document timeout, recovery, stale-base defaults, Homebrew
usage, and that this is a Harness-local profile rather than OpenAI-core conformant.

- [ ] **Step 4: Run complete verification**

Run:

```bash
cargo fmt -p harness-symphony --check
cargo test -p harness-symphony -- --nocapture
cargo clippy -p harness-symphony -- -D warnings
git diff --check
scripts/validate-harness-macos-kit.sh
```

Expected: all commands pass. If the kit validator requires a missing packaged
fixture, build the debug/release binary and web assets using its documented
prerequisites, then rerun it.
