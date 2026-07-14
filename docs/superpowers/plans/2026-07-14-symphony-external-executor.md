# Symphony External Executor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a main-agent-owned external execution lifecycle to Symphony while preserving worktree isolation, managed-adapter recovery, canonical result outcomes, and the single-active-run invariant.

**Architecture:** Extend `run_state` with an execution discriminator and prepared-database digest, then put external lease transitions behind focused state-store methods. A new `external` application module owns start, heartbeat, and complete; it reuses `RunEventWriter` and a shared run finalizer instead of duplicating adapter validation. Active-run reads reconcile expired external leases transactionally, while managed PID reconciliation remains unchanged.

**Tech Stack:** Rust 2021, clap, rusqlite/SQLite, serde, sha2, React/TypeScript, Playwright, Bash and PowerShell installer validation.

---

## File Structure

- Create `crates/harness-symphony/src/harness_digest.rs`: deterministic logical digest of a Harness SQLite database.
- Create `crates/harness-symphony/src/external.rs`: external lifecycle application service and errors.
- Modify `crates/harness-symphony/src/main.rs`: register the two focused modules.
- Modify `crates/harness-symphony/src/config.rs`: external heartbeat TTL config and validation.
- Modify `crates/harness-symphony/src/state.rs`: additive columns, typed external transitions, expiry reconciliation, and active-read integration.
- Modify `crates/harness-symphony/src/run.rs`: record the prepared digest and expose one shared finalization path.
- Modify `crates/harness-symphony/src/interface.rs`: clap commands and human-readable output.
- Modify `crates/harness-symphony/src/web.rs`: external-safe startup reconciliation, periodic expiry sweep, stale review/retry behavior, and executor response coverage.
- Modify `crates/harness-symphony/src/work.rs`: derive stale runs as Needs Attention.
- Modify `crates/harness-symphony/src/cleanup.rs`: retain stale worktrees under failed-worktree policy.
- Modify `crates/harness-symphony/web-ui/src/features/symphony/detail.tsx`: label the existing `agent` field as Executor and keep normalized external events readable.
- Modify `crates/harness-symphony/web-ui/e2e/symphony.spec.ts`: browser proof for executor and stale state.
- Modify `AGENTS.md`, `docs/SYMPHONY_QUICKSTART.md`, `scripts/harness-install-files.txt`, `scripts/validate-install-payload.sh`, and `scripts/install-harness.{sh,ps1}` only where required to distribute the approved operating guidance.

### Task 1: Add external runtime configuration

**Files:**
- Modify: `crates/harness-symphony/src/config.rs`
- Modify: `crates/harness-symphony/src/interface.rs`

- [ ] **Step 1: Write failing config tests**

Add tests beside existing config tests:

```rust
#[test]
fn external_heartbeat_ttl_defaults_to_120_seconds() {
    let resolved = SymphonyConfig::default().resolve(Path::new("/repo"));
    assert_eq!(resolved.external_heartbeat_ttl_seconds, 120);
}

#[test]
fn external_heartbeat_ttl_must_be_positive() {
    let error = serde_yaml::from_str::<SymphonyConfig>(
        "version: 1\nruns:\n  external_heartbeat_ttl_seconds: 0\n",
    )
    .unwrap_err();
    assert!(error.to_string().contains("external_heartbeat_ttl_seconds must be greater than zero"));
}
```

- [ ] **Step 2: Run the focused tests and confirm RED**

Run: `cargo test -p harness-symphony config::tests::external_heartbeat_ttl -- --nocapture`

Expected: compilation fails because the config field does not exist.

- [ ] **Step 3: Add the typed setting**

Add the following field to `ResolvedConfig` and `RunsConfig`, using the same positive-u32 deserializer pattern as `agent.timeout_minutes`:

```rust
pub const DEFAULT_EXTERNAL_HEARTBEAT_TTL_SECONDS: u32 = 120;

#[serde(
    default = "default_external_heartbeat_ttl_seconds",
    deserialize_with = "deserialize_external_heartbeat_ttl_seconds"
)]
pub external_heartbeat_ttl_seconds: u32,
```

Resolve it into `ResolvedConfig`, include it in `RunsConfig::default`, and print it from `config show` as:

```rust
println!(
    "external_heartbeat_ttl_seconds: {}",
    config.external_heartbeat_ttl_seconds
);
```

- [ ] **Step 4: Run focused config tests**

Run: `cargo test -p harness-symphony config::tests -- --nocapture`

Expected: all config tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/harness-symphony/src/config.rs crates/harness-symphony/src/interface.rs
git commit -m "feat(symphony): configure external heartbeat ttl"
```

### Task 2: Add a stable Harness database digest

**Files:**
- Create: `crates/harness-symphony/src/harness_digest.rs`
- Modify: `crates/harness-symphony/src/main.rs`
- Modify: `crates/harness-symphony/Cargo.toml`
- Modify: `Cargo.lock`

- [ ] **Step 1: Write digest tests first**

Create the module with tests covering identical databases, different insertion order, read-only opens, and one durable row change:

```rust
#[test]
fn logical_digest_ignores_insertion_order_but_detects_content_change() {
    let temp = tempfile::tempdir().unwrap();
    let first = fixture_db(&temp.path().join("first.db"), &[(1, "a"), (2, "b")]);
    let second = fixture_db(&temp.path().join("second.db"), &[(2, "b"), (1, "a")]);
    assert_eq!(logical_digest(&first).unwrap(), logical_digest(&second).unwrap());

    Connection::open(&second)
        .unwrap()
        .execute("UPDATE fixture SET value='changed' WHERE id=2", [])
        .unwrap();
    assert_ne!(logical_digest(&first).unwrap(), logical_digest(&second).unwrap());
}
```

- [ ] **Step 2: Run the module test and confirm RED**

Run: `cargo test -p harness-symphony harness_digest -- --nocapture`

Expected: compilation fails because `logical_digest` is not implemented.

- [ ] **Step 3: Implement canonical hashing**

Add `sha2 = "0.10"`. Implement:

```rust
pub fn logical_digest(path: &Path) -> Result<String, DigestError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut hasher = Sha256::new();
    for table in durable_tables(&connection)? {
        hasher.update(table.as_bytes());
        let columns = table_columns(&connection, &table)?;
        hasher.update(columns.join("\0").as_bytes());
        let mut rows = canonical_rows(&connection, &table, &columns)?;
        rows.sort();
        for row in rows {
            hasher.update((row.len() as u64).to_be_bytes());
            hasher.update(row);
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}
```

`durable_tables` must sort table names and exclude only `sqlite_%`. Encode each SQLite value with an explicit type tag (`null`, integer, real bits, UTF-8 text bytes, blob bytes) and length prefix so concatenations cannot collide. Include `sqlite_master.sql` for each durable table so schema changes affect the digest.

- [ ] **Step 4: Run digest tests**

Run: `cargo test -p harness-symphony harness_digest -- --nocapture`

Expected: all digest tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/harness-symphony/Cargo.toml Cargo.lock crates/harness-symphony/src/main.rs crates/harness-symphony/src/harness_digest.rs
git commit -m "feat(symphony): fingerprint prepared harness databases"
```

### Task 3: Extend run state and implement transactional external leases

**Files:**
- Modify: `crates/harness-symphony/src/state.rs`

- [ ] **Step 1: Add failing migration and transition tests**

Cover legacy migration defaults, external start, repeat-start rejection, managed heartbeat rejection, expiry boundary, heartbeat-before-expiry, heartbeat-after-expiry, and late completion not changing a newer active row. The core expiry assertion is:

```rust
let expired = store.reconcile_expired_external_runs(1_000, 120).unwrap();
assert_eq!(expired, vec!["run_external"]);
let run = store.show_run("run_external").unwrap();
assert_eq!(run.status, "stale");
assert_eq!(run.terminal_reason.as_deref(), Some("external heartbeat lease expired"));
```

- [ ] **Step 2: Run state tests and confirm RED**

Run: `cargo test -p harness-symphony state::tests::external -- --nocapture`

Expected: compilation fails because external state fields and methods do not exist.

- [ ] **Step 3: Add additive columns and record fields**

Add to `RunRecord`:

```rust
pub execution_mode: String,
pub harness_db_digest: Option<String>,
```

Migrate with:

```sql
ALTER TABLE run_state ADD COLUMN execution_mode TEXT NOT NULL DEFAULT 'managed';
ALTER TABLE run_state ADD COLUMN harness_db_digest TEXT;
```

Update every run SELECT and `run_from_row` consistently. Keep inserts defaulting to `managed`; add `set_harness_db_digest(run_id, digest)` for preparation.

- [ ] **Step 4: Implement guarded transitions**

Add explicit state errors for invalid status/mode. Implement these signatures:

```rust
pub fn start_external(&self, run_id: &str, executor: &str, now: i64) -> Result<(), StateError>;
pub fn heartbeat_external(&self, run_id: &str, now: i64) -> Result<(), StateError>;
pub fn reconcile_expired_external_runs(&self, now: i64, ttl_seconds: u32) -> Result<Vec<String>, StateError>;
```

Each method must use an immediate transaction. `start_external` verifies that the run is the current active prepared row before setting `status='running'`, `execution_mode='external'`, `agent`, `heartbeat_at`, and `current_stage='agent'`. Reconciliation updates only rows matching:

```sql
status='running' AND execution_mode='external'
AND heartbeat_at IS NOT NULL AND heartbeat_at + ?1 <= ?2
```

Heartbeat updates only `running external`; it never revives `stale`.

- [ ] **Step 5: Make active-dependent reads reconcile through an explicit wrapper**

Do not hide clock/config inside `RunStateStore`. Add an application-level helper later; state methods remain deterministic. Preserve `active_run()` for callers that already reconciled.

- [ ] **Step 6: Run all state tests**

Run: `cargo test -p harness-symphony state::tests -- --nocapture`

Expected: all state tests pass, including existing managed PID and queue lease tests.

- [ ] **Step 7: Commit**

```bash
git add crates/harness-symphony/src/state.rs
git commit -m "feat(symphony): add external run lease state"
```

### Task 4: Share finalization and enforce digest-backed changesets

**Files:**
- Modify: `crates/harness-symphony/src/run.rs`
- Modify: `crates/harness-symphony/src/changeset.rs`

- [ ] **Step 1: Write failing run validation tests**

Add cases for unchanged DB without changeset, changed DB with matching semantic changeset, changed DB without changeset, mismatched changeset header, and preservation of `blocked`, `partial`, and `cancelled` outcomes.

```rust
let error = finalize_prepared_run(&config, prepared).unwrap_err();
assert!(error.to_string().contains("copied harness.db changed without a valid run changeset"));
assert_eq!(store.show_run("run_changed").unwrap().status, "failed");
```

- [ ] **Step 2: Run focused tests and confirm RED**

Run: `cargo test -p harness-symphony run::tests::prepared_digest -- --nocapture`

Expected: tests fail because digest validation and shared finalization are absent.

- [ ] **Step 3: Record the digest during preparation**

After copying the root DB, calculate `logical_digest(&harness_db_path)` and persist it with `set_harness_db_digest`. Treat digest failure as preparation failure and clean resources through the existing rollback path.

- [ ] **Step 4: Expose changeset parsing without duplicating it**

Move or expose the existing JSONL header/parser logic through:

```rust
pub fn validate_run_changeset(path: &Path, expected_run_id: &str) -> Result<usize, ChangesetError>;
```

Return the number of semantic operations after the header. Reject a missing/mismatched header and zero operations when the copied DB digest changed.

- [ ] **Step 5: Extract shared finalization**

Refactor `execute_prepared_run` so adapter and external paths both call:

```rust
pub(crate) fn finalize_prepared_run(
    config: &ResolvedConfig,
    prepared: PreparedRun,
) -> Result<CompletedRun, RunError>;
```

This function sets stage `validation`, checks the current digest against the stored baseline, validates the required changeset, calls the existing artifact validator, and persists `completed.outcome` unchanged. Adapter execution remains responsible only for launching the agent before calling this function.

- [ ] **Step 6: Run run and changeset tests**

Run:

```bash
cargo test -p harness-symphony run::tests -- --nocapture
cargo test -p harness-symphony changeset::tests -- --nocapture
```

Expected: all focused tests pass and existing adapter outcome tests remain green.

- [ ] **Step 7: Commit**

```bash
git add crates/harness-symphony/src/run.rs crates/harness-symphony/src/changeset.rs
git commit -m "feat(symphony): share digest-backed run finalization"
```

### Task 5: Implement external lifecycle application commands

**Files:**
- Create: `crates/harness-symphony/src/external.rs`
- Modify: `crates/harness-symphony/src/main.rs`
- Modify: `crates/harness-symphony/src/interface.rs`

- [ ] **Step 1: Write failing application and clap tests**

Test start guards, bounded executor/step strings, heartbeat event deduplication policy, stale completion, managed-run rejection, and clap parsing for all three commands.

```rust
assert!(Cli::try_parse_from([
    "harness-symphony", "runs", "heartbeat", "run_1", "--step", "tests passing"
]).is_ok());
```

- [ ] **Step 2: Run focused tests and confirm RED**

Run:

```bash
cargo test -p harness-symphony external -- --nocapture
cargo test -p harness-symphony interface::tests -- --nocapture
```

Expected: command variants and external module are missing.

- [ ] **Step 3: Implement the application boundary**

Expose:

```rust
pub fn reconcile_external_runs(config: &ResolvedConfig) -> Result<Vec<String>, ExternalError>;
pub fn start(config: &ResolvedConfig, run_id: &str, executor: &str) -> Result<RunRecord, ExternalError>;
pub fn heartbeat(config: &ResolvedConfig, run_id: &str, step: Option<&str>) -> Result<RunRecord, ExternalError>;
pub fn complete(config: &ResolvedConfig, run_id: &str) -> Result<CompletedRun, ExternalError>;
```

Use a shared `unix_timestamp()` helper and `RunEventWriter` at `config.runs_dir.join(run_id).join("RUN_EVENTS.jsonl")`. Start emits one lifecycle event. Heartbeat without `--step` emits none; a bounded changed step emits one progress event. Complete reconstructs `PreparedRun` from durable state, rejects lightweight/managed/prepared rows, and delegates to `finalize_prepared_run`.

- [ ] **Step 4: Add clap variants and output**

Add `Start`, `Heartbeat`, and `Complete` to `RunsAction`. Before any active-dependent command (`Run`, `Status`, `Runs`, `Work Board`, cleanup, and Web reads), call `reconcile_external_runs`. Print executor, status, heartbeat, and completed artifact paths without exposing internal SQLite paths.

- [ ] **Step 5: Run CLI tests**

Run:

```bash
cargo test -p harness-symphony external -- --nocapture
cargo test -p harness-symphony interface -- --nocapture
```

Expected: all focused tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/harness-symphony/src/main.rs crates/harness-symphony/src/external.rs crates/harness-symphony/src/interface.rs
git commit -m "feat(symphony): add external lifecycle commands"
```

### Task 6: Integrate recovery, board state, cleanup, and Web presentation

**Files:**
- Modify: `crates/harness-symphony/src/web.rs`
- Modify: `crates/harness-symphony/src/work.rs`
- Modify: `crates/harness-symphony/src/cleanup.rs`
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/detail.tsx`
- Modify: `crates/harness-symphony/web-ui/e2e/symphony.spec.ts`

- [ ] **Step 1: Write failing Rust recovery tests**

Add tests proving Web startup preserves a live external row, expires an old external row without invoking the process terminator, leaves managed PID recovery unchanged, maps stale to Needs Attention/retry, and retains stale worktrees according to failed-worktree policy.

- [ ] **Step 2: Run focused Rust tests and confirm RED**

Run:

```bash
cargo test -p harness-symphony web::tests::external -- --nocapture
cargo test -p harness-symphony work::tests::stale -- --nocapture
cargo test -p harness-symphony cleanup::tests::stale -- --nocapture
```

Expected: stale and execution mode are not integrated.

- [ ] **Step 3: Split managed and external reconciliation**

At Web startup call `reconcile_external_runs(config)` first. Filter PID reconciliation with:

```rust
.filter(|run| {
    run.execution_mode == "managed"
        && matches!(run.status.as_str(), "prepared" | "running")
})
```

Spawn one named thread when the Web server starts; every five seconds it calls external reconciliation and logs failures without stopping the server. Do not spawn one timer per request.

- [ ] **Step 4: Integrate stale derivation and cleanup**

Add `stale` to Needs Attention and execution retry classifications. Add stale to failed-worktree retention with an explicit `ExpiredStale` cleanup reason. Do not include stale in `active_run`.

- [ ] **Step 5: Write failing browser test**

Mock a review with `agent: "claude-subagent"` and a normalized external milestone event. Assert the detail displays `Executor`, `Claude Subagent`, and the milestone. Add a stale board fixture and assert Needs Attention rather than Ready.

- [ ] **Step 6: Implement the minimal UI copy**

Reuse `ReviewResponse.agent`; add no executor API field. In the review metadata render:

```tsx
<Field label="Executor" value={agentLabel(review.agent)} />
```

Keep `EventLog` on normalized events and update the raw-artifact label to `RUN_EVENTS.jsonl` when normalized events are present.

- [ ] **Step 7: Run Web and browser validation**

Run:

```bash
cargo test -p harness-symphony web -- --nocapture
cargo test -p harness-symphony work -- --nocapture
cargo test -p harness-symphony cleanup -- --nocapture
npm --prefix crates/harness-symphony/web-ui run build
npm --prefix crates/harness-symphony/web-ui run e2e
```

Expected: Rust tests pass, TypeScript build passes, and all Playwright tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/harness-symphony/src/web.rs crates/harness-symphony/src/work.rs crates/harness-symphony/src/cleanup.rs crates/harness-symphony/web-ui/src/features/symphony/detail.tsx crates/harness-symphony/web-ui/e2e/symphony.spec.ts
git commit -m "feat(symphony): surface external executor recovery"
```

### Task 7: Ship reusable operating guidance in fresh installs

**Files:**
- Modify: `AGENTS.md`
- Modify: `docs/SYMPHONY_QUICKSTART.md`
- Modify: `scripts/harness-install-files.txt`
- Modify: `scripts/validate-install-payload.sh`
- Modify: `scripts/install-harness.sh`
- Modify: `scripts/install-harness.ps1`

- [ ] **Step 1: Add failing payload assertions**

Extend `scripts/validate-install-payload.sh` to require the fresh target to contain `docs/SYMPHONY_QUICKSTART.md` and both installed `AGENTS.md` and Quickstart to mention `runs start`, `runs heartbeat`, and `runs complete`. Keep existing assertions that source stories, decisions, databases, and run history are absent.

- [ ] **Step 2: Run payload validation and confirm RED**

Run: `scripts/validate-install-payload.sh`

Expected: failure because Quickstart is not in `scripts/harness-install-files.txt` and the lifecycle guidance is absent.

- [ ] **Step 3: Update operating docs and shared installer shims**

Document this exact ownership rule in both installer-generated agent shims:

```text
For an approved external-executor story, the main agent runs prepare-only,
start, periodic heartbeat, and complete from the source repository. The
subagent edits only inside the printed worktree and never invokes root
lifecycle commands.
```

Add `docs/SYMPHONY_QUICKSTART.md` to the manifest. Document the 30-second maximum heartbeat interval, 120-second default TTL, stale recovery, late completion, and explicit `--repo-root` usage.

- [ ] **Step 4: Run fresh-install proof**

Run: `scripts/validate-install-payload.sh`

Expected: pass for fresh install, merge install, refresh-agent-shim, and both Bash/PowerShell manifest guards.

- [ ] **Step 5: Commit**

```bash
git add AGENTS.md docs/SYMPHONY_QUICKSTART.md scripts/harness-install-files.txt scripts/validate-install-payload.sh scripts/install-harness.sh scripts/install-harness.ps1
git commit -m "docs(harness): ship external executor guidance"
```

### Task 8: Full verification and real lifecycle smoke

**Files:**
- Modify: `docs/stories/epics/E08-symphony-web-ui-controller/US-094-symphony-external-executor/validation.md`
- Modify: `.harness/runs/$HARNESS_RUN_ID/SUMMARY.md` and `RESULT.json` inside the Symphony worktree only

- [ ] **Step 1: Run Rust quality gates**

```bash
cargo fmt --check
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Expected: all commands exit zero with no failed tests or warnings.

- [ ] **Step 2: Run UI and platform gates**

```bash
npm --prefix crates/harness-symphony/web-ui run build
npm --prefix crates/harness-symphony/web-ui run e2e
npm --prefix crates/harness-symphony/web-ui run desktop:smoke
scripts/validate-install-payload.sh
```

Expected: build, Playwright, Electron smoke, and payload validation all pass.

- [ ] **Step 3: Run deterministic CLI lifecycle smoke in a temporary repository**

Prepare one fixture story, call `run --prepare-only`, parse its run/worktree paths, then run start, heartbeat with a milestone, write valid SUMMARY/RESULT artifacts in that worktree, and call complete. Assert `runs show` reports `execution_mode: external`, `agent: claude-subagent`, and `status: completed`; assert `RUN_EVENTS.jsonl` contains the milestone.

- [ ] **Step 4: Run stale and late-completion smoke**

Use a temporary config with a short positive TTL, start an external run, advance through real waiting only in the temporary smoke, trigger reconciliation with `status`, and assert stale releases the lock. Prepare a newer run, then complete the stale run and assert the newer run remains active.

- [ ] **Step 5: Update durable validation evidence**

Replace `Pending implementation` in the story validation file with exact command results and fixture run IDs. Record proof:

```bash
scripts/bin/harness-cli story update --id US-094 --status implemented \
  --unit 1 --integration 1 --e2e 1 --platform 1 \
  --evidence "cargo test --workspace; cargo fmt --check; cargo clippy --workspace -- -D warnings; web build/e2e; desktop smoke; install-payload validation; external lifecycle and stale/late-completion smokes"
scripts/bin/harness-cli story verify US-094
```

- [ ] **Step 6: Final diff and commit**

```bash
git diff --check
git status --short
git add docs/stories/epics/E08-symphony-web-ui-controller/US-094-symphony-external-executor/validation.md
git commit -m "test(symphony): verify external executor lifecycle"
```

## Plan Self-Review

- Spec coverage: lifecycle commands, control-plane boundary, lock semantics, execution modes, TTL reconciliation, canonical outcomes, digest-backed changesets, UI/events, stale cleanup, docs, fresh install, and manual proof each map to a task.
- Placeholder scan: implementation steps contain no deferred placeholders; runtime IDs use the existing `HARNESS_RUN_ID` environment variable.
- Type consistency: `execution_mode`, `harness_db_digest`, `external_heartbeat_ttl_seconds`, `start_external`, `heartbeat_external`, `reconcile_expired_external_runs`, `logical_digest`, and `finalize_prepared_run` use the same names throughout.
