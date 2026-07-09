# Benchmark Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the deterministic single-artifact scoring engine of the Benchmark Harness — given one captured agent run plus its expected-proof spec, score functional + compliance checks and roll the results up by harness responsibility (the H3 attribution primitive).

**Architecture:** A new `harness-bench` workspace crate. This plan implements only the **score half** (deterministic, hermetic) from `docs/superpowers/specs/2026-07-09-benchmark-harness-design.md`. It reads a captured *artifact* directory (produced code worktree + optional `harness.db`) and a TOML *task spec* (the expected proof), runs each check, and prints per-check results plus a responsibility rollup. Compliance checks read the artifact's SQLite `harness.db` directly with `rusqlite` (no subprocess, fully testable). Functional checks run the task's `test_command` in the artifact worktree.

**Tech Stack:** Rust (edition 2021), clap 4 (derive), rusqlite 0.39 (bundled SQLite), serde/serde_json, toml 0.8, thiserror 2. Dev: tempfile.

## Global Constraints

- Rust edition `2021`; workspace resolver `3` (from root `Cargo.toml`).
- Crate name: `harness-bench`. Binary name: `harness-bench`.
- Dependency versions pinned to match the sibling `harness-cli` crate: `clap = { version = "4.6.1", features = ["derive"] }`, `rusqlite = { version = "0.39.0", features = ["bundled"] }`, `serde_json = "1.0.145"`, `thiserror = "2.0.18"`.
- No network, no reliance on the gitignored `scripts/bin/harness-cli` binary in this plan. All tests are hermetic (tempfile + rusqlite).
- Deterministic only: no LLM-judge, no live agent.
- Arm identifiers are the lowercase strings `"h0"` (bare) and `"hn"` (harnessed).
- Commit style: Conventional Commits. Work happens on the existing `benchmark-harness` git branch.

## Scope: three-plan decomposition

This initiative is split along the design's capture⟂score seam so each plan ships working, testable software:

1. **This plan** — single-artifact score engine: task-spec parser, artifact reader, functional + compliance scorers, responsibility rollup, `score` CLI. Hermetic + deterministic.
2. **Next plan** — aggregation + report: K-run pass-rate/median/range, H0−Hn functional delta, responsibility scorecard, delta-vs-previous, JSON+Markdown reports, plus the quality scorers that shell out to `harness-cli score-trace` / `score-context`.
3. **Later plan** — capture half: Rust fixture app, arm build from the installer manifest, agent adapter, capture orchestration, real tasks T1/T2/T4.

## File Structure

- `Cargo.toml` (root) — add `crates/harness-bench` to `members`.
- `crates/harness-bench/Cargo.toml` — crate manifest.
- `crates/harness-bench/src/main.rs` — clap CLI entry; `score` subcommand.
- `crates/harness-bench/src/lib.rs` — module declarations + re-exports.
- `crates/harness-bench/src/error.rs` — `BenchError`.
- `crates/harness-bench/src/task.rs` — `TaskSpec`, `Functional`, `Check` (TOML).
- `crates/harness-bench/src/artifact.rs` — `Artifact`, `Meta` (JSON).
- `crates/harness-bench/src/score.rs` — functional + compliance scorers, `RunScore`.
- `crates/harness-bench/src/responsibility.rs` — rollup by responsibility.
- `crates/harness-bench/tests/score_cli.rs` — end-to-end integration test of the `score` subcommand.

Each source file has one responsibility: parsing task specs, reading artifacts, scoring, aggregating tags, and CLI wiring respectively.

---

### Task 1: Crate scaffolding + TaskSpec parser

**Files:**
- Modify: `Cargo.toml` (root, `members` array at line 3)
- Create: `crates/harness-bench/Cargo.toml`
- Create: `crates/harness-bench/src/lib.rs`
- Create: `crates/harness-bench/src/error.rs`
- Create: `crates/harness-bench/src/main.rs`
- Create: `crates/harness-bench/src/task.rs`

**Interfaces:**
- Produces: `harness_bench::error::BenchError` (enum, `thiserror`); `harness_bench::task::{TaskSpec, Functional, Check}`; `TaskSpec::from_toml_str(&str) -> Result<TaskSpec, BenchError>` and `TaskSpec::load(&Path) -> Result<TaskSpec, BenchError>`.

- [ ] **Step 1: Write the failing test**

Create `crates/harness-bench/src/task.rs`:

```rust
use std::path::Path;

use serde::Deserialize;

use crate::error::BenchError;

/// A benchmark task's expected-proof spec, loaded from `expected.toml`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TaskSpec {
    pub id: String,
    pub lane: String,
    pub functional: Functional,
    #[serde(default)]
    pub checks: Vec<Check>,
}

/// The cross-arm functional check: a shell command whose exit status is pass/fail.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Functional {
    pub test_command: String,
}

/// One harness-only check, tagged with the responsibility it measures.
///
/// `kind` selects the check type: `"sql_expect"` compares the first column of
/// `sql` to `expect`; `"sql_nonzero"` passes when the first column of `sql` is
/// a count greater than zero.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Check {
    pub id: String,
    pub responsibility: String,
    pub kind: String,
    #[serde(default)]
    pub sql: Option<String>,
    #[serde(default)]
    pub expect: Option<String>,
}

impl TaskSpec {
    pub fn from_toml_str(text: &str) -> Result<Self, BenchError> {
        toml::from_str(text).map_err(|e| BenchError::TaskParse(e.to_string()))
    }

    pub fn load(path: &Path) -> Result<Self, BenchError> {
        let text = std::fs::read_to_string(path).map_err(|source| BenchError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_toml_str(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
id = "T1"
lane = "tiny"

[functional]
test_command = "cargo test"

[[checks]]
id = "intake_lane"
responsibility = "Task specification"
kind = "sql_expect"
sql = "SELECT lane FROM intake ORDER BY id DESC LIMIT 1"
expect = "tiny"

[[checks]]
id = "trace_recorded"
responsibility = "Observability"
kind = "sql_nonzero"
sql = "SELECT count(*) FROM trace"
"#;

    #[test]
    fn parses_task_spec_fields() {
        let spec = TaskSpec::from_toml_str(SAMPLE).expect("valid spec parses");
        assert_eq!(spec.id, "T1");
        assert_eq!(spec.lane, "tiny");
        assert_eq!(spec.functional.test_command, "cargo test");
        assert_eq!(spec.checks.len(), 2);
        assert_eq!(spec.checks[0].id, "intake_lane");
        assert_eq!(spec.checks[0].responsibility, "Task specification");
        assert_eq!(spec.checks[0].kind, "sql_expect");
        assert_eq!(spec.checks[0].expect.as_deref(), Some("tiny"));
        assert_eq!(spec.checks[1].kind, "sql_nonzero");
    }

    #[test]
    fn rejects_malformed_toml() {
        let err = TaskSpec::from_toml_str("id = ").unwrap_err();
        assert!(matches!(err, BenchError::TaskParse(_)));
    }
}
```

Create the supporting scaffolding so the test can compile.

`crates/harness-bench/Cargo.toml`:

```toml
[package]
name = "harness-bench"
version = "0.1.0"
edition = "2021"
license = "MIT"

[[bin]]
name = "harness-bench"
path = "src/main.rs"

[dependencies]
clap = { version = "4.6.1", features = ["derive"] }
rusqlite = { version = "0.39.0", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1.0.145"
toml = "0.8"
thiserror = "2.0.18"

[dev-dependencies]
tempfile = "3"
```

`crates/harness-bench/src/error.rs`:

```rust
/// Errors surfaced by the benchmark scoring engine.
#[derive(Debug, thiserror::Error)]
pub enum BenchError {
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse task spec: {0}")]
    TaskParse(String),

    #[error("failed to parse artifact meta: {0}")]
    MetaParse(String),

    #[error("artifact missing required entry: {0}")]
    ArtifactMissing(String),

    #[error("sqlite error: {0}")]
    Sqlite(String),

    #[error("check '{0}' misconfigured: {1}")]
    CheckConfig(String, String),
}
```

`crates/harness-bench/src/lib.rs`:

```rust
pub mod artifact;
pub mod error;
pub mod responsibility;
pub mod score;
pub mod task;
```

`crates/harness-bench/src/main.rs` (minimal, expanded in Task 5):

```rust
fn main() {
    std::process::exit(0);
}
```

Add empty placeholder modules so `lib.rs` compiles this task (they are filled in later tasks):

`crates/harness-bench/src/artifact.rs`:

```rust
// Filled in Task 2.
```

`crates/harness-bench/src/score.rs`:

```rust
// Filled in Task 3.
```

`crates/harness-bench/src/responsibility.rs`:

```rust
// Filled in Task 4.
```

Modify root `Cargo.toml` line 3:

```toml
members = ["crates/harness-cli", "crates/harness-symphony", "crates/harness-bench"]
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p harness-bench task:: 2>&1 | tail -20`
Expected: FAIL — the empty `artifact.rs`/`score.rs`/`responsibility.rs` compile, but if any type is referenced before creation the compile fails; once compiling, the two `task` tests run. On first write they should PASS (parser is complete). If they do not compile, fix the module stubs. The intent of this step is to confirm the test is wired and runs.

Note: because `task.rs` is complete here, expect the two `task::tests` to PASS. Treat a compile error as the "failing" signal to resolve.

- [ ] **Step 3: Confirm implementation is present**

`task.rs` above already contains the implementation. No extra code needed.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p harness-bench task:: 2>&1 | tail -20`
Expected: PASS — `parses_task_spec_fields` and `rejects_malformed_toml` both pass.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/harness-bench
git commit -m "feat(bench): scaffold harness-bench crate and task-spec parser"
```

---

### Task 2: Artifact reader

**Files:**
- Modify: `crates/harness-bench/src/artifact.rs`
- Test: inline `#[cfg(test)]` in the same file

**Interfaces:**
- Consumes: `BenchError`.
- Produces: `harness_bench::artifact::{Artifact, Meta}`; `Artifact::load(&Path) -> Result<Artifact, BenchError>`. `Artifact` fields: `dir: PathBuf`, `meta: Meta`, `worktree: PathBuf`, `harness_db: Option<PathBuf>`. `Meta` fields: `task: String`, `arm: String`, `k: u32`, `agent: String`, `exit_code: i32`.

- [ ] **Step 1: Write the failing test**

Replace `crates/harness-bench/src/artifact.rs` with:

```rust
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::BenchError;

/// Metadata describing one captured run, read from `meta.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Meta {
    pub task: String,
    /// "h0" (bare) or "hn" (harnessed).
    pub arm: String,
    pub k: u32,
    pub agent: String,
    pub exit_code: i32,
}

/// A captured run on disk: produced worktree + optional harness database.
#[derive(Debug, Clone, PartialEq)]
pub struct Artifact {
    pub dir: PathBuf,
    pub meta: Meta,
    pub worktree: PathBuf,
    pub harness_db: Option<PathBuf>,
}

impl Artifact {
    /// Load an artifact directory. Requires `meta.json` and a `worktree/`
    /// directory; `harness.db` is optional (absent on the bare arm).
    pub fn load(dir: &Path) -> Result<Self, BenchError> {
        let meta_path = dir.join("meta.json");
        let meta_text = std::fs::read_to_string(&meta_path).map_err(|source| BenchError::Io {
            path: meta_path.display().to_string(),
            source,
        })?;
        let meta: Meta =
            serde_json::from_str(&meta_text).map_err(|e| BenchError::MetaParse(e.to_string()))?;

        let worktree = dir.join("worktree");
        if !worktree.is_dir() {
            return Err(BenchError::ArtifactMissing(worktree.display().to_string()));
        }

        let db = dir.join("harness.db");
        let harness_db = if db.is_file() { Some(db) } else { None };

        Ok(Artifact {
            dir: dir.to_path_buf(),
            meta,
            worktree,
            harness_db,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_meta(dir: &Path, arm: &str) {
        let meta = format!(
            r#"{{"task":"T1","arm":"{arm}","k":1,"agent":"fake","exit_code":0}}"#
        );
        fs::write(dir.join("meta.json"), meta).unwrap();
        fs::create_dir_all(dir.join("worktree")).unwrap();
    }

    #[test]
    fn loads_hn_artifact_with_db() {
        let tmp = tempfile::tempdir().unwrap();
        write_meta(tmp.path(), "hn");
        fs::write(tmp.path().join("harness.db"), b"").unwrap();

        let artifact = Artifact::load(tmp.path()).unwrap();
        assert_eq!(artifact.meta.arm, "hn");
        assert_eq!(artifact.meta.task, "T1");
        assert_eq!(artifact.worktree, tmp.path().join("worktree"));
        assert_eq!(artifact.harness_db, Some(tmp.path().join("harness.db")));
    }

    #[test]
    fn bare_artifact_has_no_db() {
        let tmp = tempfile::tempdir().unwrap();
        write_meta(tmp.path(), "h0");
        let artifact = Artifact::load(tmp.path()).unwrap();
        assert!(artifact.harness_db.is_none());
    }

    #[test]
    fn missing_worktree_errors() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("meta.json"),
            r#"{"task":"T1","arm":"h0","k":1,"agent":"fake","exit_code":0}"#,
        )
        .unwrap();
        let err = Artifact::load(tmp.path()).unwrap_err();
        assert!(matches!(err, BenchError::ArtifactMissing(_)));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p harness-bench artifact:: 2>&1 | tail -20`
Expected: On first write the tests compile and PASS (implementation is included). If a compile error appears, resolve it — that is the failing signal.

- [ ] **Step 3: Implementation present**

Included above. No extra code.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p harness-bench artifact:: 2>&1 | tail -20`
Expected: PASS — all three artifact tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/harness-bench/src/artifact.rs
git commit -m "feat(bench): read captured artifact directories"
```

---

### Task 3: Functional + compliance scorers

**Files:**
- Modify: `crates/harness-bench/src/score.rs`
- Test: inline `#[cfg(test)]` in the same file

**Interfaces:**
- Consumes: `TaskSpec`, `Check` (Task 1); `Artifact` (Task 2); `BenchError`.
- Produces: `harness_bench::score::{CheckResult, RunScore}`; `functional_pass(&TaskSpec, &Artifact) -> Result<bool, BenchError>`; `run_checks(&TaskSpec, &Artifact) -> Result<Vec<CheckResult>, BenchError>`; `score_artifact(&TaskSpec, &Artifact) -> Result<RunScore, BenchError>`. `CheckResult` fields: `id: String`, `responsibility: String`, `passed: bool`, `detail: String`. `RunScore` fields: `task: String`, `arm: String`, `k: u32`, `functional: bool`, `checks: Vec<CheckResult>`.

- [ ] **Step 1: Write the failing test**

Replace `crates/harness-bench/src/score.rs` with:

```rust
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::artifact::Artifact;
use crate::error::BenchError;
use crate::task::{Check, TaskSpec};

/// Result of one harness-only check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckResult {
    pub id: String,
    pub responsibility: String,
    pub passed: bool,
    pub detail: String,
}

/// Full score for one captured run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunScore {
    pub task: String,
    pub arm: String,
    pub k: u32,
    pub functional: bool,
    pub checks: Vec<CheckResult>,
}

/// Run the task's functional command in the artifact worktree.
/// Pass iff the command exits 0.
pub fn functional_pass(spec: &TaskSpec, artifact: &Artifact) -> Result<bool, BenchError> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(&spec.functional.test_command)
        .current_dir(&artifact.worktree)
        .status()
        .map_err(|source| BenchError::Io {
            path: artifact.worktree.display().to_string(),
            source,
        })?;
    Ok(status.success())
}

fn query_first_string(db: &Path, sql: &str) -> Result<Option<String>, BenchError> {
    let conn =
        rusqlite::Connection::open(db).map_err(|e| BenchError::Sqlite(e.to_string()))?;
    let value = conn.query_row(sql, [], |row| row.get::<_, String>(0));
    match value {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(BenchError::Sqlite(e.to_string())),
    }
}

fn query_first_i64(db: &Path, sql: &str) -> Result<i64, BenchError> {
    let conn =
        rusqlite::Connection::open(db).map_err(|e| BenchError::Sqlite(e.to_string()))?;
    conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        .map_err(|e| BenchError::Sqlite(e.to_string()))
}

fn eval_check(check: &Check, artifact: &Artifact) -> Result<(bool, String), BenchError> {
    // Every current check kind needs the harness database.
    let db = match &artifact.harness_db {
        Some(db) => db,
        None => return Ok((false, "no harness.db (bare arm)".to_string())),
    };

    match check.kind.as_str() {
        "sql_expect" => {
            let sql = check
                .sql
                .as_deref()
                .ok_or_else(|| BenchError::CheckConfig(check.id.clone(), "missing sql".into()))?;
            let expect = check.expect.as_deref().ok_or_else(|| {
                BenchError::CheckConfig(check.id.clone(), "missing expect".into())
            })?;
            let actual = query_first_string(db, sql)?;
            match actual {
                Some(v) if v == expect => Ok((true, format!("{v}"))),
                Some(v) => Ok((false, format!("got '{v}', want '{expect}'"))),
                None => Ok((false, format!("no rows, want '{expect}'"))),
            }
        }
        "sql_nonzero" => {
            let sql = check
                .sql
                .as_deref()
                .ok_or_else(|| BenchError::CheckConfig(check.id.clone(), "missing sql".into()))?;
            let count = query_first_i64(db, sql)?;
            Ok((count > 0, format!("count={count}")))
        }
        other => Err(BenchError::CheckConfig(
            check.id.clone(),
            format!("unknown kind '{other}'"),
        )),
    }
}

pub fn run_checks(spec: &TaskSpec, artifact: &Artifact) -> Result<Vec<CheckResult>, BenchError> {
    let mut results = Vec::with_capacity(spec.checks.len());
    for check in &spec.checks {
        let (passed, detail) = eval_check(check, artifact)?;
        results.push(CheckResult {
            id: check.id.clone(),
            responsibility: check.responsibility.clone(),
            passed,
            detail,
        });
    }
    Ok(results)
}

pub fn score_artifact(spec: &TaskSpec, artifact: &Artifact) -> Result<RunScore, BenchError> {
    let functional = functional_pass(spec, artifact)?;
    let checks = run_checks(spec, artifact)?;
    Ok(RunScore {
        task: spec.id.clone(),
        arm: artifact.meta.arm.clone(),
        k: artifact.meta.k,
        functional,
        checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::Artifact;
    use crate::task::TaskSpec;
    use std::fs;
    use std::path::Path;

    /// Build a harness.db with an `intake(lane)` row and `trace` rows.
    fn seed_db(path: &Path, lane: &str, traces: usize) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE intake(id INTEGER PRIMARY KEY, lane TEXT);
             CREATE TABLE trace(id INTEGER PRIMARY KEY, summary TEXT);",
        )
        .unwrap();
        conn.execute("INSERT INTO intake(lane) VALUES (?1)", [lane])
            .unwrap();
        for i in 0..traces {
            conn.execute("INSERT INTO trace(summary) VALUES (?1)", [format!("t{i}")])
                .unwrap();
        }
    }

    fn artifact_with_db(exit_ok: bool, lane: &str, traces: usize) -> (tempfile::TempDir, Artifact) {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("worktree")).unwrap();
        fs::write(
            tmp.path().join("meta.json"),
            r#"{"task":"T1","arm":"hn","k":1,"agent":"fake","exit_code":0}"#,
        )
        .unwrap();
        let _ = exit_ok;
        seed_db(&tmp.path().join("harness.db"), lane, traces);
        let artifact = Artifact::load(tmp.path()).unwrap();
        (tmp, artifact)
    }

    fn spec_with(test_command: &str) -> TaskSpec {
        let toml = format!(
            r#"
id = "T1"
lane = "tiny"

[functional]
test_command = "{test_command}"

[[checks]]
id = "intake_lane"
responsibility = "Task specification"
kind = "sql_expect"
sql = "SELECT lane FROM intake ORDER BY id DESC LIMIT 1"
expect = "tiny"

[[checks]]
id = "trace_recorded"
responsibility = "Observability"
kind = "sql_nonzero"
sql = "SELECT count(*) FROM trace"
"#
        );
        TaskSpec::from_toml_str(&toml).unwrap()
    }

    #[test]
    fn functional_passes_on_exit_zero() {
        let (_tmp, artifact) = artifact_with_db(true, "tiny", 1);
        let spec = spec_with("true");
        assert!(functional_pass(&spec, &artifact).unwrap());
    }

    #[test]
    fn functional_fails_on_exit_nonzero() {
        let (_tmp, artifact) = artifact_with_db(true, "tiny", 1);
        let spec = spec_with("false");
        assert!(!functional_pass(&spec, &artifact).unwrap());
    }

    #[test]
    fn sql_expect_matches_and_mismatches() {
        let (_tmp, artifact) = artifact_with_db(true, "tiny", 1);
        let results = run_checks(&spec_with("true"), &artifact).unwrap();
        assert!(results[0].passed, "lane matches 'tiny'");

        let (_tmp2, wrong) = artifact_with_db(true, "high-risk", 1);
        let results = run_checks(&spec_with("true"), &wrong).unwrap();
        assert!(!results[0].passed, "lane 'high-risk' != 'tiny'");
    }

    #[test]
    fn sql_nonzero_reflects_trace_count() {
        let (_tmp, has) = artifact_with_db(true, "tiny", 3);
        assert!(run_checks(&spec_with("true"), &has).unwrap()[1].passed);

        let (_tmp2, none) = artifact_with_db(true, "tiny", 0);
        assert!(!run_checks(&spec_with("true"), &none).unwrap()[1].passed);
    }

    #[test]
    fn checks_fail_closed_without_db() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("worktree")).unwrap();
        fs::write(
            tmp.path().join("meta.json"),
            r#"{"task":"T1","arm":"h0","k":1,"agent":"fake","exit_code":0}"#,
        )
        .unwrap();
        let artifact = Artifact::load(tmp.path()).unwrap();
        let results = run_checks(&spec_with("true"), &artifact).unwrap();
        assert!(results.iter().all(|r| !r.passed));
        assert!(results[0].detail.contains("no harness.db"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p harness-bench score:: 2>&1 | tail -25`
Expected: The file is complete, so tests compile and PASS. A compile error is the failing signal to fix.

- [ ] **Step 3: Implementation present**

Included above.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p harness-bench score:: 2>&1 | tail -25`
Expected: PASS — five score tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/harness-bench/src/score.rs
git commit -m "feat(bench): functional and compliance scorers"
```

---

### Task 4: Responsibility rollup

**Files:**
- Modify: `crates/harness-bench/src/responsibility.rs`
- Test: inline `#[cfg(test)]` in the same file

**Interfaces:**
- Consumes: `CheckResult` (Task 3).
- Produces: `harness_bench::responsibility::{ResponsibilityScore, rollup}`; `rollup(&[CheckResult]) -> std::collections::BTreeMap<String, ResponsibilityScore>`. `ResponsibilityScore` fields: `passed: u32`, `total: u32`.

- [ ] **Step 1: Write the failing test**

Replace `crates/harness-bench/src/responsibility.rs` with:

```rust
use std::collections::BTreeMap;

use serde::Serialize;

use crate::score::CheckResult;

/// Passed/total tally for one harness responsibility.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct ResponsibilityScore {
    pub passed: u32,
    pub total: u32,
}

/// Group check results by their responsibility tag, tallying pass/total.
/// `BTreeMap` keeps responsibilities in stable alphabetical order for output.
pub fn rollup(checks: &[CheckResult]) -> BTreeMap<String, ResponsibilityScore> {
    let mut map: BTreeMap<String, ResponsibilityScore> = BTreeMap::new();
    for check in checks {
        let entry = map.entry(check.responsibility.clone()).or_default();
        entry.total += 1;
        if check.passed {
            entry.passed += 1;
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(responsibility: &str, passed: bool) -> CheckResult {
        CheckResult {
            id: "c".into(),
            responsibility: responsibility.into(),
            passed,
            detail: String::new(),
        }
    }

    #[test]
    fn tallies_pass_and_total_per_responsibility() {
        let checks = vec![
            result("Observability", true),
            result("Observability", false),
            result("Task specification", true),
        ];
        let map = rollup(&checks);
        assert_eq!(map["Observability"], ResponsibilityScore { passed: 1, total: 2 });
        assert_eq!(
            map["Task specification"],
            ResponsibilityScore { passed: 1, total: 1 }
        );
    }

    #[test]
    fn empty_input_is_empty_map() {
        assert!(rollup(&[]).is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p harness-bench responsibility:: 2>&1 | tail -20`
Expected: compiles and PASS. Compile error = failing signal.

- [ ] **Step 3: Implementation present**

Included above.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p harness-bench responsibility:: 2>&1 | tail -20`
Expected: PASS — both rollup tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/harness-bench/src/responsibility.rs
git commit -m "feat(bench): roll up check results by responsibility"
```

---

### Task 5: `score` CLI subcommand

**Files:**
- Modify: `crates/harness-bench/src/main.rs`

**Interfaces:**
- Consumes: `TaskSpec::load`, `Artifact::load`, `score::score_artifact`, `responsibility::rollup`.
- Produces: CLI `harness-bench score --artifact <DIR> --task <FILE>` that prints a stable, assertable text report and exits 0 on success.

- [ ] **Step 1: Write the failing test**

Create `crates/harness-bench/tests/score_cli.rs`:

```rust
use std::fs;
use std::process::Command;

/// End-to-end: build a fake harnessed artifact and its task spec, run the
/// `score` subcommand, and assert the printed report.
#[test]
fn score_subcommand_prints_rollup() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // Artifact dir: meta.json + worktree + seeded harness.db.
    let artifact = root.join("artifact");
    fs::create_dir_all(artifact.join("worktree")).unwrap();
    fs::write(
        artifact.join("meta.json"),
        r#"{"task":"T1","arm":"hn","k":1,"agent":"fake","exit_code":0}"#,
    )
    .unwrap();
    let conn = rusqlite::Connection::open(artifact.join("harness.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE intake(id INTEGER PRIMARY KEY, lane TEXT);
         CREATE TABLE trace(id INTEGER PRIMARY KEY, summary TEXT);
         INSERT INTO intake(lane) VALUES ('tiny');
         INSERT INTO trace(summary) VALUES ('did work');",
    )
    .unwrap();
    drop(conn);

    // Task spec.
    let task = root.join("expected.toml");
    fs::write(
        &task,
        r#"
id = "T1"
lane = "tiny"

[functional]
test_command = "true"

[[checks]]
id = "intake_lane"
responsibility = "Task specification"
kind = "sql_expect"
sql = "SELECT lane FROM intake ORDER BY id DESC LIMIT 1"
expect = "tiny"

[[checks]]
id = "trace_recorded"
responsibility = "Observability"
kind = "sql_nonzero"
sql = "SELECT count(*) FROM trace"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_harness-bench"))
        .arg("score")
        .arg("--artifact")
        .arg(&artifact)
        .arg("--task")
        .arg(&task)
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("functional: PASS"), "stdout was:\n{stdout}");
    assert!(stdout.contains("[PASS] intake_lane"), "stdout was:\n{stdout}");
    assert!(stdout.contains("[PASS] trace_recorded"), "stdout was:\n{stdout}");
    assert!(stdout.contains("Task specification"), "stdout was:\n{stdout}");
    assert!(stdout.contains("1/1"), "stdout was:\n{stdout}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p harness-bench --test score_cli 2>&1 | tail -20`
Expected: FAIL — `main.rs` still exits 0 without parsing args or printing anything; assertions on stdout content fail.

- [ ] **Step 3: Implement the CLI**

Replace `crates/harness-bench/src/main.rs` with:

```rust
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use harness_bench::artifact::Artifact;
use harness_bench::responsibility::rollup;
use harness_bench::score::score_artifact;
use harness_bench::task::TaskSpec;

#[derive(Parser)]
#[command(name = "harness-bench", about = "Benchmark harness scoring engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Score a single captured artifact against its task spec.
    Score {
        /// Path to the captured artifact directory.
        #[arg(long)]
        artifact: PathBuf,
        /// Path to the task's expected.toml spec.
        #[arg(long)]
        task: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Commands::Score { artifact, task } => match run_score(&artifact, &task) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
    }
}

fn run_score(
    artifact_dir: &std::path::Path,
    task_path: &std::path::Path,
) -> Result<(), harness_bench::error::BenchError> {
    let spec = TaskSpec::load(task_path)?;
    let artifact = Artifact::load(artifact_dir)?;
    let score = score_artifact(&spec, &artifact)?;

    println!("Task {} | arm {} | k {}", score.task, score.arm, score.k);
    println!(
        "  functional: {}",
        if score.functional { "PASS" } else { "FAIL" }
    );
    println!("  checks:");
    for c in &score.checks {
        println!(
            "    [{}] {} ({}): {}",
            if c.passed { "PASS" } else { "FAIL" },
            c.id,
            c.responsibility,
            c.detail
        );
    }
    println!("  responsibility rollup:");
    for (responsibility, tally) in rollup(&score.checks) {
        println!("    {:<24} {}/{}", responsibility, tally.passed, tally.total);
    }
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p harness-bench --test score_cli 2>&1 | tail -20`
Expected: PASS — the printed report contains `functional: PASS`, both `[PASS]` check lines, `Task specification`, and `1/1`.

- [ ] **Step 5: Commit**

```bash
git add crates/harness-bench/src/main.rs crates/harness-bench/tests/score_cli.rs
git commit -m "feat(bench): add score subcommand"
```

---

### Task 6: Whole-crate verification gate

**Files:**
- None (verification only).

**Interfaces:**
- Consumes: everything above.

- [ ] **Step 1: Full test run**

Run: `cargo test -p harness-bench 2>&1 | tail -20`
Expected: PASS — all unit tests (task, artifact, score, responsibility) and the `score_cli` integration test pass.

- [ ] **Step 2: Format + lint gate**

Run: `cargo fmt -p harness-bench --check && cargo clippy -p harness-bench -- -D warnings 2>&1 | tail -20`
Expected: no formatting diff; clippy reports no warnings.

- [ ] **Step 3: Workspace still builds**

Run: `cargo build 2>&1 | tail -20`
Expected: the full workspace (harness-cli, harness-symphony, harness-bench) builds.

- [ ] **Step 4: Commit any fmt fixes**

```bash
git add -A
git commit -m "chore(bench): fmt and clippy clean" || echo "nothing to commit"
```

---

## Self-Review

**Spec coverage (against the design doc's score half):**
- Task-spec = expected-proof model → Task 1 (`TaskSpec`, checks tagged with responsibility).
- Artifact reader (worktree + optional harness.db, arm field) → Task 2.
- Functional (cross-arm, run test_command) → Task 3 `functional_pass`.
- Compliance (query harness.db, expected-vs-actual + existence) → Task 3 `sql_expect` / `sql_nonzero`.
- Bare arm fails harness-only checks closed → Task 3 `checks_fail_closed_without_db`.
- Responsibility attribution rollup → Task 4.
- CLI surface → Task 5.
- Deferred (explicitly, to Plans 2/3): trace/context quality scorers (shell `score-trace`/`score-context`), K-run aggregation, H0−Hn delta, reports, capture, real fixture app. Listed in "Scope" section — not gaps.

**Placeholder scan:** No TBD/TODO. The empty module stubs in Task 1 are filled by Tasks 2–4, and each is created before it is referenced by `lib.rs` so the crate compiles at every task boundary.

**Type consistency:** `CheckResult` (Task 3) is consumed with the same field names by `rollup` (Task 4) and printed in Task 5. `RunScore` fields (`task`, `arm`, `k`, `functional`, `checks`) match between Task 3 definition and Task 5 usage. `Artifact`/`Meta` fields match between Task 2 and Task 3/5. `TaskSpec`/`Check` fields (`kind`, `sql`, `expect`, `responsibility`) match between Task 1 and Task 3's `eval_check`.

**Note on TDD shape:** because each module is written complete-with-tests in one task, several tasks' "verify it fails" step manifests as a compile error rather than a red assertion. Task 5 is the exception — a true red→green against the CLI binary. This is a deliberate trade for hermetic, self-contained modules; the reviewer gate still runs at each task boundary.
