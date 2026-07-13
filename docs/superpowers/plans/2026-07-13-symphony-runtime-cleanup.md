# Symphony Runtime Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Make installed Harness projects ignore Symphony runtime state and safely reclaim terminal worktrees and old terminal-run evidence without deleting branches, active work, or permanent changesets.

**Architecture:** Add a focused cleanup module that derives eligible worktrees from local Symphony state plus guarded orphan discovery, removes only paths contained by the configured worktree root, and reports every outcome. Keep run-evidence compaction in retention, make it state-aware, and invoke both engines from the explicit CLI, Web startup, and successful sync lifecycle while treating automatic cleanup failures as warnings.

**Tech Stack:** Rust 2021, clap, rusqlite, std filesystem/process APIs, temporary Git-repository tests, Bash and PowerShell installers.

---

## File Structure

- Create crates/harness-symphony/src/cleanup.rs for candidate discovery, retention decisions, path safety, Git worktree removal, byte accounting, and reports.
- Modify crates/harness-symphony/src/main.rs to register the cleanup module.
- Modify crates/harness-symphony/src/config.rs for operational defaults and seven-day retention.
- Modify crates/harness-symphony/src/state.rs for a cleanup-only state projection; do not change the SQLite schema.
- Modify crates/harness-symphony/src/retention.rs so compaction only removes proven terminal evidence.
- Modify crates/harness-symphony/src/interface.rs and web.rs for CLI, startup, sync, warnings, and cleaned-state display.
- Modify scripts/install-harness.sh, install-harness.ps1, and validate-install-payload.sh for runtime ignore rules.
- Modify docs/stories/US-092-symphony-runtime-cleanup.md only after validation passes.

### Task 1: Make Cleanup Configuration Operational

**Files:**
- Modify: crates/harness-symphony/src/config.rs
- Modify fixtures: crates/harness-symphony/src/{agent,auto,doctor,pr,retention,run,sync,web}.rs

- [ ] **Step 1: Write failing config tests**

In the default-resolution test, assert:

~~~rust
assert!(resolved.keep_failed_worktrees);
assert!(resolved.cleanup_after_sync);
assert_eq!(resolved.failed_worktree_retention_days, 7);
~~~

Extend the YAML parsing fixture with:

~~~yaml
cleanup:
  keep_failed_worktrees: false
  cleanup_after_sync: false
  failed_worktree_retention_days: 3
~~~

Assert the three resolved override values.

- [ ] **Step 2: Verify the tests fail**

Run: cargo test -p harness-symphony config::tests -- --nocapture

Expected: compilation fails because failed_worktree_retention_days is absent, and the old cleanup_after_sync default assertion disagrees.

- [ ] **Step 3: Implement the typed contract**

Add:

~~~rust
pub struct ResolvedConfig {
    // existing fields remain
    pub keep_failed_worktrees: bool,
    pub cleanup_after_sync: bool,
    pub failed_worktree_retention_days: u32,
}

pub struct CleanupConfig {
    #[serde(default = "default_true")]
    pub keep_failed_worktrees: bool,
    #[serde(default = "default_true")]
    pub cleanup_after_sync: bool,
    #[serde(default = "default_failed_worktree_retention_days")]
    pub failed_worktree_retention_days: u32,
}

fn default_failed_worktree_retention_days() -> u32 {
    7
}
~~~

Set CleanupConfig::default to true/true/7 and copy all three fields in SymphonyConfig::resolve. Add failed_worktree_retention_days: 7 to every ResolvedConfig test fixture returned by:

~~~bash
rg -l 'ResolvedConfig \{' crates/harness-symphony/src
~~~

- [ ] **Step 4: Run config tests and compile every fixture**

Run: cargo test -p harness-symphony config::tests -- --nocapture

Expected: all config tests pass.

Run: cargo test -p harness-symphony --no-run

Expected: compilation succeeds.

- [ ] **Step 5: Commit**

~~~bash
git add crates/harness-symphony/src
git commit -m "feat: configure Symphony runtime cleanup"
~~~

### Task 2: Add a Cleanup-Safe State Projection

**Files:**
- Modify: crates/harness-symphony/src/state.rs

- [ ] **Step 1: Write the failing projection test**

Create a failed NewRunRecord, backdate it in the temporary state DB, then assert:

~~~rust
connection.execute(
    "UPDATE run_state SET updated_at=datetime('now', '-8 days') WHERE run_id='run_old';",
    [],
).unwrap();

let runs = store.list_cleanup_runs().unwrap();
assert_eq!(runs.len(), 1);
assert_eq!(runs[0].run_id, "run_old");
assert_eq!(runs[0].status, "failed");
assert_eq!(runs[0].sync_status, "not_applied");
assert!(runs[0].updated_at_epoch > 0);
~~~

- [ ] **Step 2: Verify failure**

Run: cargo test -p harness-symphony state::tests::lists_cleanup_run_state_with_update_age -- --nocapture

Expected: compilation fails because list_cleanup_runs is undefined.

- [ ] **Step 3: Implement the projection**

Add:

~~~rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupRunRecord {
    pub run_id: String,
    pub worktree: PathBuf,
    pub lightweight: bool,
    pub status: String,
    pub sync_status: String,
    pub updated_at_epoch: i64,
}

pub fn list_cleanup_runs(&self) -> Result<Vec<CleanupRunRecord>, StateError> {
    self.init()?;
    let connection = Connection::open(&self.path)?;
    let mut statement = connection.prepare(
        "SELECT run_id, worktree, lightweight, status, sync_status,
                CAST(strftime('%s', updated_at) AS INTEGER)
         FROM run_state ORDER BY updated_at ASC, run_id ASC;",
    )?;
    statement
        .query_map([], |row| Ok(CleanupRunRecord {
            run_id: row.get(0)?,
            worktree: PathBuf::from(row.get::<_, String>(1)?),
            lightweight: row.get::<_, i64>(2)? != 0,
            status: row.get(3)?,
            sync_status: row.get(4)?,
            updated_at_epoch: row.get(5)?,
        }))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StateError::from)
}
~~~

Do not add a migration and do not enlarge RunRecord.

- [ ] **Step 4: Run state tests**

Run: cargo test -p harness-symphony state::tests -- --nocapture

Expected: all state tests pass.

- [ ] **Step 5: Commit**

~~~bash
git add crates/harness-symphony/src/state.rs
git commit -m "feat: expose cleanup run state"
~~~

### Task 3: Build the Guarded Worktree Cleanup Engine

**Files:**
- Create: crates/harness-symphony/src/cleanup.rs
- Modify: crates/harness-symphony/src/main.rs

- [ ] **Step 1: Write failing real-Git tests**

Create a Fixture helper that initializes a temporary Git repository, makes real git worktree add registrations, writes run state, and can backdate updated_at. Add:

~~~rust
#[test]
fn done_runs_clean_immediately_but_preserve_branch() {
    let fixture = Fixture::with_worktree("run_done", "completed", "synced", 0);
    let result = cleanup_runtime_at(&fixture.config, false, fixture.now).unwrap();
    assert_eq!(result.removed_count(), 1);
    assert!(!fixture.worktree("run_done").exists());
    assert!(fixture.branch_exists("symphony/run_done"));
}

#[test]
fn active_recent_failed_and_external_paths_are_preserved() {
    let fixture = Fixture::mixed_candidates();
    let result = cleanup_runtime_at(&fixture.config, false, fixture.now).unwrap();
    assert_eq!(result.removed_count(), 0);
    assert!(fixture.active_worktree().exists());
    assert!(fixture.recent_failed_worktree().exists());
    assert!(fixture.external_directory().exists());
}

#[test]
fn expired_failed_and_orphan_worktrees_are_removed() {
    let fixture = Fixture::expired_candidates(8);
    let result = cleanup_runtime_at(&fixture.config, false, fixture.now).unwrap();
    assert_eq!(result.removed_count(), 2);
}

#[test]
fn dry_run_reports_without_removing() {
    let fixture = Fixture::with_worktree("run_done", "completed", "synced", 0);
    let result = cleanup_runtime_at(&fixture.config, true, fixture.now).unwrap();
    assert_eq!(result.candidates(), 1);
    assert_eq!(result.removed_count(), 0);
    assert!(fixture.worktree("run_done").exists());
}

#[test]
fn repeated_cleanup_is_idempotent() {
    let fixture = Fixture::with_worktree("run_done", "completed", "synced", 0);
    cleanup_runtime_at(&fixture.config, false, fixture.now).unwrap();
    let second = cleanup_runtime_at(&fixture.config, false, fixture.now).unwrap();
    assert_eq!(second.failures(), 0);
    assert!(!fixture.worktree("run_done").exists());
}
~~~

- [ ] **Step 2: Verify failure**

Run: cargo test -p harness-symphony cleanup::tests -- --nocapture

Expected: compilation fails because the module and contracts do not exist.

- [ ] **Step 3: Define the public report**

~~~rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupReason {
    Done,
    ExpiredFailed,
    ExpiredInterrupted,
    ExpiredCancelled,
    Orphan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupItem {
    pub run_id: Option<String>,
    pub path: PathBuf,
    pub reason: CleanupReason,
    pub removed: bool,
    pub reclaimed_bytes: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CleanupResult {
    pub items: Vec<CleanupItem>,
}

impl CleanupResult {
    pub fn candidates(&self) -> usize { self.items.len() }
    pub fn removed_count(&self) -> usize {
        self.items.iter().filter(|item| item.removed).count()
    }
    pub fn failures(&self) -> usize {
        self.items.iter().filter(|item| item.error.is_some()).count()
    }
    pub fn reclaimed_bytes(&self) -> u64 {
        self.items.iter().map(|item| item.reclaimed_bytes).sum()
    }
}
~~~

CleanupError covers discovery, state, and git-list failures. Per-item deletion failures go in CleanupItem.error so later candidates still run.

- [ ] **Step 4: Implement deterministic eligibility**

~~~rust
fn eligible_reason(
    run: &CleanupRunRecord,
    config: &ResolvedConfig,
    now: i64,
) -> Option<CleanupReason> {
    if run.lightweight || matches!(run.status.as_str(), "prepared" | "running") {
        return None;
    }
    if run.status == "completed"
        && run.sync_status == "synced"
        && config.cleanup_after_sync
    {
        return Some(CleanupReason::Done);
    }
    let ttl = i64::from(config.failed_worktree_retention_days) * 86_400;
    let expired = !config.keep_failed_worktrees
        || now.saturating_sub(run.updated_at_epoch) >= ttl;
    if !expired {
        return None;
    }
    match run.status.as_str() {
        "failed" => Some(CleanupReason::ExpiredFailed),
        "interrupted" => Some(CleanupReason::ExpiredInterrupted),
        "cancelled" => Some(CleanupReason::ExpiredCancelled),
        _ => None,
    }
}
~~~

For orphans, inspect only direct children named run_* under worktrees_dir, use directory modification time, and exclude every path referenced by prepared/running state.

- [ ] **Step 5: Implement containment and deletion**

Reject symlink entries through symlink_metadata before canonicalization. Require:

~~~rust
fn contained_candidate(root: &Path, candidate: &Path) -> Result<PathBuf, CleanupError> {
    let root = root.canonicalize()?;
    let candidate = candidate.canonicalize()?;
    if candidate == root || !candidate.starts_with(&root) {
        return Err(CleanupError::UnsafePath(candidate));
    }
    Ok(candidate)
}
~~~

If the candidate no longer exists, report it as already absent with zero bytes
and no error. This makes retry after a completed or partially completed cleanup
idempotent.

Measure bytes recursively without following symlinks. For each safe candidate:

1. Run git worktree remove --force PATH from repo_root.
2. If Git says the directory is unregistered, use remove_dir_all only after containment succeeds.
3. Run git worktree prune after the candidate loop.
4. Never invoke git branch -D.

- [ ] **Step 6: Run focused tests and lint**

Run: cargo test -p harness-symphony cleanup::tests -- --nocapture

Expected: all cleanup tests pass.

Run: cargo clippy -p harness-symphony -- -D warnings

Expected: no warnings.

- [ ] **Step 7: Commit**

~~~bash
git add crates/harness-symphony/src/cleanup.rs crates/harness-symphony/src/main.rs
git commit -m "feat: safely clean Symphony worktrees"
~~~

### Task 4: Protect Active Evidence During Compaction

**Files:**
- Modify: crates/harness-symphony/src/retention.rs

- [ ] **Step 1: Write failing evidence tests**

Create run_active, run_terminal, and run_unknown folders; state contains only active and terminal records. Assert:

~~~rust
let result = compact_runs(&config, 1).unwrap();
assert!(config.runs_dir.join("run_active").exists());
assert!(config.runs_dir.join("run_unknown").exists());
assert!(result.kept.contains(&config.runs_dir.join("run_terminal")));
~~~

Create three terminal folders with keep_last 1. Assert exactly two are removed and .harness/changesets/run_old.changeset.jsonl survives.

- [ ] **Step 2: Verify the old algorithm fails**

Run: cargo test -p harness-symphony retention::tests -- --nocapture

Expected: the current directory-only algorithm tries to compact active or unknown folders.

- [ ] **Step 3: Filter through state**

Load RunStateStore::list_cleanup_runs into a map. A folder is eligible only when a matching state record exists and:

~~~rust
fn terminal_for_compaction(run: &CleanupRunRecord) -> bool {
    !matches!(run.status.as_str(), "prepared" | "running")
}
~~~

Unknown folders remain untouched because terminal state cannot be proven. Preserve the keep_last >= 1 guard and never traverse changeset_directory.

- [ ] **Step 4: Run retention and cleanup tests**

Run: cargo test -p harness-symphony retention::tests -- --nocapture

Expected: terminal evidence is bounded; active, unknown, and changeset files remain.

- [ ] **Step 5: Commit**

~~~bash
git add crates/harness-symphony/src/retention.rs
git commit -m "fix: preserve active Symphony run evidence"
~~~

### Task 5: Wire CLI, Startup, Sync, And Display

**Files:**
- Modify: crates/harness-symphony/src/interface.rs
- Modify: crates/harness-symphony/src/web.rs

- [ ] **Step 1: Write failing CLI tests**

Parse:

~~~rust
let cli = Cli::try_parse_from([
    "harness-symphony", "runs", "cleanup", "--dry-run"
]).unwrap();
assert!(matches!(
    cli.command,
    Command::Runs(RunsArgs {
        action: RunsAction::Cleanup { dry_run: true }
    })
));
~~~

Add a pure cleanup_lines test that asserts candidate, removed, failure, reclaimed-byte, reason, and path output. Add worktree_state tests: absent terminal is cleaned; absent prepared/running is missing.

- [ ] **Step 2: Write failing lifecycle tests**

In web.rs, extend the successful sync fixture with a real worktree:

~~~rust
let response = sync_run_response(&config, "run_sync").unwrap();
assert_eq!(response.status, 200);
assert!(!config.worktrees_dir.join("run_sync").exists());
assert!(branch_exists(&config.repo_root, "symphony/run_sync"));
~~~

Add a deletion-failure fixture where sync still returns 200 and the worktree remains retryable. Add a Web startup fixture with an eight-day failed worktree and assert the sweep removes it.

- [ ] **Step 3: Verify failure**

Run: cargo test -p harness-symphony interface::tests web::tests -- --nocapture

Expected: CLI variant and lifecycle hooks are absent.

- [ ] **Step 4: Add CLI and config output**

Extend RunsAction:

~~~rust
Cleanup {
    #[arg(long)]
    dry_run: bool,
},
~~~

Dispatch through cleanup_runtime. The explicit command returns non-zero when result.failures() is non-zero. Print every candidate and summary counts. Add failed_worktree_retention_days to config show.

- [ ] **Step 5: Add best-effort hooks**

Implement:

~~~rust
pub fn cleanup_best_effort(config: &ResolvedConfig) {
    match cleanup_runtime(config, false) {
        Ok(result) => {
            for item in result.items.iter().filter(|item| item.error.is_some()) {
                eprintln!(
                    "warning: Symphony cleanup skipped {}: {}",
                    item.path.display(),
                    item.error.as_deref().unwrap_or("unknown error")
                );
            }
        }
        Err(error) => eprintln!("warning: Symphony cleanup failed: {error}"),
    }
}
~~~

Invoke cleanup and then compact_runs(config, config.compact_keep_last):

- at the beginning of Command::Run and Command::Auto, including --no-web;
- at the beginning of run_web_server;
- after sync_changeset succeeds in sync_run_response;
- after top-level sync_changesets succeeds in Command::Sync.

Warn on automatic cleanup/compaction failure. Never call cleanup after a failed sync.

- [ ] **Step 6: Render cleaned state**

~~~rust
fn worktree_state(run: &RunRecord) -> &'static str {
    if run.worktree.exists() {
        "present"
    } else if matches!(run.status.as_str(), "prepared" | "running") {
        "missing"
    } else {
        "cleaned"
    }
}
~~~

Add Worktree State to run list output and worktree_state to detail output. Preserve stored branch/path metadata.

- [ ] **Step 7: Run focused and full Symphony tests**

Run: cargo test -p harness-symphony interface::tests web::tests sync::tests retention::tests cleanup::tests -- --nocapture

Expected: all focused tests pass.

Run: cargo test -p harness-symphony

Expected: all Symphony tests pass.

- [ ] **Step 8: Commit**

~~~bash
git add crates/harness-symphony/src/interface.rs crates/harness-symphony/src/web.rs
git commit -m "feat: automate Symphony runtime cleanup"
~~~

### Task 6: Merge Runtime Ignore Rules Into Installed Projects

**Files:**
- Modify: scripts/install-harness.sh
- Modify: scripts/install-harness.ps1
- Modify: scripts/validate-install-payload.sh

- [ ] **Step 1: Add failing installer assertions**

After fresh install, loop over these exact rules and require grep -Fxc count 1:

~~~text
.symphony/
.worktrees/
.harness/*
!.harness/changesets/
!.harness/changesets/*.changeset.jsonl
~~~

Create a target-owned .gitignore containing vendor/, run --merge twice, assert vendor/ remains, and assert every Harness rule occurs once. Initialize Git, create runtime files and a changeset, then run:

~~~bash
git -C "$IGNORE_TARGET" check-ignore -q .symphony/state.db
git -C "$IGNORE_TARGET" check-ignore -q .harness/runs/run_1/RESULT.json
if git -C "$IGNORE_TARGET" check-ignore -q \
  .harness/changesets/run_1.changeset.jsonl
then
  fail "changesets must remain visible to Git"
fi
~~~

Add equivalent literal-rule checks to the existing PowerShell fixture when pwsh is available.

- [ ] **Step 2: Verify installer validation fails**

Run: scripts/validate-install-payload.sh

Expected: existing-target assertions fail because merge functions omit runtime rules.

- [ ] **Step 3: Update both merge functions**

The complete rule set is:

~~~text
# Harness durable layer
harness.db
harness.db-wal
harness.db-shm
scripts/bin/harness-cli
scripts/bin/harness-cli.exe
.symphony/
.worktrees/
.harness/*
!.harness/changesets/
!.harness/changesets/*.changeset.jsonl
~~~

Replace Bash's all-or-nothing early return with per-line missing-rule collection, matching PowerShell: append only missing lines, preserve target content, and remain idempotent.

- [ ] **Step 4: Run syntax and payload proof**

Run: bash -n scripts/install-harness.sh

Expected: exit 0.

Run: scripts/validate-install-payload.sh

Expected: fresh, merge, update, idempotence, symlink-safety, Git visibility, and available PowerShell checks pass.

- [ ] **Step 5: Commit**

~~~bash
git add scripts/install-harness.sh scripts/install-harness.ps1 \
  scripts/validate-install-payload.sh
git commit -m "fix: ignore installed Symphony runtime state"
~~~

### Task 7: Release Verification And Durable Evidence

**Files:**
- Modify: docs/stories/US-092-symphony-runtime-cleanup.md
- Modify only if implementation changed the approved contract: docs/SYMPHONY_SCOPE.md

- [ ] **Step 1: Run release validation**

Run: cargo fmt --check

Expected: exit 0.

Run: cargo clippy --workspace -- -D warnings

Expected: exit 0 with no warnings.

Run: cargo test --workspace

Expected: all tests pass.

Run: scripts/validate-install-payload.sh

Expected: all template/fresh-install checks pass.

Run: git diff --check

Expected: exit 0.

- [ ] **Step 2: Run a temporary-repository smoke**

Create a temporary Git repository with completed/synced, eight-day failed, active, and orphan worktrees plus one changeset. Run:

~~~bash
target/debug/harness-symphony --repo-root "$SMOKE_REPO" \
  runs cleanup --dry-run
target/debug/harness-symphony --repo-root "$SMOKE_REPO" runs cleanup
git -C "$SMOKE_REPO" worktree list --porcelain
git -C "$SMOKE_REPO" branch --list 'symphony/*'
~~~

Expected: dry-run changes nothing; apply removes Done, expired failed, and orphan candidates; active work remains; branches and changesets remain.

- [ ] **Step 3: Record exact evidence and verify US-092**

Replace the story's current Evidence sentence with actual commands and observed counts, then run:

~~~bash
scripts/bin/harness-cli story update --id US-092 --status implemented \
  --unit 1 --integration 1 --e2e 0 --platform 1 \
  --evidence "Cleanup tests; temporary Git smoke; installer payload validation; workspace test/fmt/clippy."
scripts/bin/harness-cli story verify US-092
~~~

Expected: story verification passes.

- [ ] **Step 4: Record the implementation trace**

~~~bash
scripts/bin/harness-cli trace \
  --summary "Implemented safe Symphony runtime cleanup" \
  --intake 30 --story US-092 --agent symphony --outcome completed \
  --actions "Added guarded worktree cleanup, terminal-only compaction, lifecycle hooks, CLI reporting, and installer ignore rules." \
  --read "docs/superpowers/specs/2026-07-13-symphony-runtime-cleanup-design.md,docs/superpowers/plans/2026-07-13-symphony-runtime-cleanup.md" \
  --changed "crates/harness-symphony/src,scripts/install-harness.sh,scripts/install-harness.ps1,scripts/validate-install-payload.sh,docs/stories/US-092-symphony-runtime-cleanup.md" \
  --decisions "Branches and changesets remain; only terminal contained worktrees and terminal evidence are eligible." \
  --errors none --friction none
~~~

Expected: trace meets the normal-lane standard tier.

- [ ] **Step 5: Commit final evidence**

~~~bash
git add docs/SYMPHONY_SCOPE.md \
  docs/stories/US-092-symphony-runtime-cleanup.md
git commit -m "docs: record Symphony cleanup validation"
~~~

## Completion Gate

- Installed projects receive every runtime ignore rule exactly once.
- Done cleanup removes worktrees but preserves branches and changesets.
- Failed, interrupted, cancelled, and orphan worktrees obey the seven-day default.
- Prepared/running worktrees and evidence are never removed.
- Explicit cleanup reports failure and exits non-zero; automatic cleanup warns and retries without changing successful sync state.
- US-092 verification and every release command pass.
