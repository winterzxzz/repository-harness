# US-093 Review Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve every valid review thread on PR #23, preserve the existing runtime contract, and merge only after fresh verification.

**Architecture:** Keep the existing file-backed normalized event stream, but move setup to writer construction, emit RFC 3339 timestamps, batch compaction, and atomically replace compacted files. Harden controller recovery at the process-group boundary and preserve primary run failures when terminal-state persistence also fails.

**Tech Stack:** Rust 2021, `time` 0.3 RFC 3339 formatting/parsing, `tempfile` 3.27 atomic persist, SQLite state, GitHub GraphQL review threads.

---

### Task 1: Event writer initialization and timestamp contract

**Files:**
- Modify: `crates/harness-symphony/Cargo.toml`
- Modify: `crates/harness-symphony/src/run_events.rs`

- [ ] **Step 1: Write failing timestamp and initialization tests**

Add tests that require writer construction to create its parent and emitted timestamps to parse as RFC 3339:

```rust
#[test]
fn writer_initialization_creates_event_directory() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("nested/run/RUN_EVENTS.jsonl");

    RunEventWriter::new(path, "codex").unwrap();

    assert!(temp.path().join("nested/run").is_dir());
}

#[test]
fn event_timestamp_is_rfc3339() {
    use time::{format_description::well_known::Rfc3339, OffsetDateTime};

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("RUN_EVENTS.jsonl");
    let event = RunEventWriter::new(path, "codex")
        .unwrap()
        .append("message", "agent", "hello")
        .unwrap();

    OffsetDateTime::parse(&event.timestamp, &Rfc3339).unwrap();
}
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```bash
cargo test -p harness-symphony writer_initialization_creates_event_directory event_timestamp_is_rfc3339 -- --nocapture
```

Because Cargo accepts one filter at a time, run each test separately. Expected failures: the parent does not exist after construction and parsing the Unix epoch string as RFC 3339 fails.

- [ ] **Step 3: Add the timestamp dependency and minimal implementation**

Add:

```toml
time = { version = "0.3", features = ["formatting", "parsing"] }
```

Create the parent before reading the existing page and replace `unix_timestamp` with:

```rust
fn rfc3339_timestamp() -> std::io::Result<String> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(std::io::Error::other)
}
```

Set `timestamp: rfc3339_timestamp()?` and remove the append-time `create_dir_all` call.

- [ ] **Step 4: Run both focused tests and verify GREEN**

Run each test filter from Step 2. Expected: both pass.

- [ ] **Step 5: Commit the event contract fix**

```bash
git add Cargo.lock crates/harness-symphony/Cargo.toml crates/harness-symphony/src/run_events.rs
git commit -m "fix(symphony): align run event timestamp contract"
```

### Task 2: Batched and atomic event compaction

**Files:**
- Modify: `crates/harness-symphony/src/run_events.rs`

- [ ] **Step 1: Write failing compaction policy tests**

Add a pure policy test and a replacement-artifact assertion:

```rust
#[test]
fn compaction_is_batched_after_limit() {
    assert!(!should_compact(2_001, 2_000));
    assert!(should_compact(2_100, 2_000));
    assert!(should_compact(4, 2));
}

#[test]
fn compaction_does_not_leave_temporary_files() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("RUN_EVENTS.jsonl");
    let writer = RunEventWriter::with_limit(path, "codex", 2).unwrap();
    for message in ["one", "two", "three"] {
        writer.append("message", "agent", message).unwrap();
    }
    let names = std::fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(names, vec![std::ffi::OsString::from("RUN_EVENTS.jsonl")]);
}
```

- [ ] **Step 2: Run the policy test and verify RED**

Run `cargo test -p harness-symphony compaction_is_batched_after_limit -- --nocapture`.
Expected: compile failure because `should_compact` does not exist.

- [ ] **Step 3: Implement batched compaction and atomic persist**

Add:

```rust
const COMPACTION_INTERVAL: usize = 100;

fn should_compact(next_sequence: u64, max_events: usize) -> bool {
    let interval = max_events.clamp(1, COMPACTION_INTERVAL) as u64;
    next_sequence > max_events as u64 && next_sequence % interval == 0
}
```

Call `compact` only when this predicate is true. Replace `fs::write` with a same-directory temporary file:

```rust
let parent = path.parent().unwrap_or_else(|| Path::new("."));
let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
temporary.write_all(&replacement)?;
temporary.as_file_mut().sync_all()?;
temporary.persist(path).map_err(|error| error.error)?;
Ok(())
```

- [ ] **Step 4: Run all run-event tests and verify GREEN**

Run `cargo test -p harness-symphony run_events::tests -- --nocapture`.
Expected: all run-event tests pass, including stale-cursor retention.

- [ ] **Step 5: Commit the compaction fix**

```bash
git add crates/harness-symphony/src/run_events.rs
git commit -m "fix(symphony): batch atomic event compaction"
```

### Task 3: Safe and bounded startup process recovery

**Files:**
- Modify: `crates/harness-symphony/src/web.rs`

- [ ] **Step 1: Write failing PID-validation and poll-schedule tests**

Add Unix tests:

```rust
#[cfg(unix)]
#[test]
fn process_group_rejects_zero_and_unrepresentable_pid() {
    assert!(validated_process_group(0).is_err());
    assert!(validated_process_group(u32::MAX).is_err());
}

#[test]
fn zombie_probe_runs_only_on_last_wait_attempt() {
    assert!(!should_probe_zombie(0, 5));
    assert!(!should_probe_zombie(3, 5));
    assert!(should_probe_zombie(4, 5));
}
```

- [ ] **Step 2: Run focused tests and verify RED**

Run each test filter separately. Expected: compile failure because both helpers are absent.

- [ ] **Step 3: Implement validated process groups and bounded polling**

Add:

```rust
#[cfg(unix)]
fn validated_process_group(pid: u32) -> Result<i32, WebError> {
    let pid = i32::try_from(pid).map_err(|_| WebError::ProcessTermination { pid })?;
    if pid == 0 {
        return Err(WebError::ProcessTermination { pid: 0 });
    }
    Ok(-pid)
}

fn should_probe_zombie(attempt: usize, attempts: usize) -> bool {
    attempt.saturating_add(1) == attempts
}
```

Use `validated_process_group(pid)?` before either signal. Change wait sleeps to 50ms, use 5 TERM attempts and 10 KILL attempts, and call `recorded_process_is_zombie` only on the last attempt after identity still matches.

- [ ] **Step 4: Run startup recovery tests and verify GREEN**

Run:

```bash
cargo test -p harness-symphony startup_reconcil -- --nocapture
cargo test -p harness-symphony startup_termination -- --nocapture
cargo test -p harness-symphony process_group_rejects -- --nocapture
cargo test -p harness-symphony zombie_probe_runs -- --nocapture
```

Expected: all pass and the TERM-ignoring fixture is force-terminated.

- [ ] **Step 5: Commit the recovery fix**

```bash
git add crates/harness-symphony/src/web.rs
git commit -m "fix(symphony): validate recovery process groups"
```

### Task 4: Preserve primary execution and validation failures

**Files:**
- Modify: `crates/harness-symphony/src/run.rs`

- [ ] **Step 1: Write a failing primary-error preservation test**

Add:

```rust
#[test]
fn state_finish_failure_does_not_replace_primary_error() {
    let primary = RunError::InvalidResult("primary validation failure".to_owned());
    let returned = preserve_primary_error(
        primary,
        Err(StateError::RunNotFound("run_1".to_owned())),
        "validation",
    );
    assert!(returned.to_string().contains("primary validation failure"));
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run `cargo test -p harness-symphony state_finish_failure_does_not_replace_primary_error -- --nocapture`.
Expected: compile failure because `preserve_primary_error` does not exist.

- [ ] **Step 3: Implement primary-error preservation**

Add:

```rust
fn preserve_primary_error(
    primary: RunError,
    finish_result: Result<(), StateError>,
    context: &str,
) -> RunError {
    if let Err(error) = finish_result {
        eprintln!("warning: failed to persist {context} terminal state: {error}");
    }
    primary
}
```

Convert `AgentError` to `RunError` before finishing, pass the finish result to this helper, and return the helper result. Apply the same pattern to `validate_finished_run` failures. Leave successful terminal transitions unchanged.

- [ ] **Step 4: Run focused and run-module tests and verify GREEN**

Run:

```bash
cargo test -p harness-symphony state_finish_failure_does_not_replace_primary_error -- --nocapture
cargo test -p harness-symphony run::tests -- --nocapture
```

Expected: all pass.

- [ ] **Step 5: Commit the error-preservation fix**

```bash
git add crates/harness-symphony/src/run.rs
git commit -m "fix(symphony): preserve primary run failures"
```

### Task 5: Verify, publish, and close review threads

**Files:**
- Verify all modified files
- Update GitHub PR #23 review threads

- [ ] **Step 1: Run full release gates**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
scripts/validate-install-payload.sh
git diff --check
```

Expected: every command exits zero.

- [ ] **Step 2: Push the review fixes**

```bash
git push origin symphony/run_1783999475145922000_45510_0
```

- [ ] **Step 3: Reply to and resolve each review thread**

For the nine valid threads, reply with the specific fix and commit, then resolve through GraphQL. For the process-group thread, reply that `configure_process_group(&mut process)` is already called before spawn at the current head and resolve it as not applicable.

- [ ] **Step 4: Re-read PR state**

Run the bundled `fetch_comments.py` and `gh pr view 23 --json mergeable,mergeStateStatus,reviewDecision,statusCheckRollup,headRefOid`. Expected: no unresolved actionable threads and a mergeable clean head.

### Task 6: Merge PR and update the main checkout

**Files:**
- No source modifications

- [ ] **Step 1: Merge PR #23**

Run `gh pr merge 23 --merge --delete-branch=false`. Expected: GitHub reports the PR merged.

- [ ] **Step 2: Update the main checkout**

From `/Users/winterzxzz/Documents/Local/repository-harness`, verify the worktree is clean, checkout `main`, and run `git pull --ff-only origin main`.

- [ ] **Step 3: Verify merged state**

Run:

```bash
gh pr view 23 --json state,mergedAt,mergeCommit,url
git status -sb
git rev-parse HEAD
git rev-parse origin/main
```

Expected: PR state is `MERGED`, the main checkout is clean on `main`, and local/remote main SHAs match.
