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
    let status = functional_command(&spec.functional.test_command)
        .current_dir(&artifact.worktree)
        .status()
        .map_err(|source| BenchError::Io {
            path: artifact.worktree.display().to_string(),
            source,
        })?;
    Ok(status.success())
}

fn functional_command(test_command: &str) -> Command {
    #[cfg(windows)]
    let mut command = Command::new("cmd");
    #[cfg(windows)]
    command.arg("/C");

    #[cfg(not(windows))]
    let mut command = Command::new("sh");
    #[cfg(not(windows))]
    command.arg("-c");

    command.arg(test_command);
    command
}

fn query_first_string(
    conn: &rusqlite::Connection,
    sql: &str,
) -> Result<Option<String>, BenchError> {
    let value = conn.query_row(sql, [], |row| row.get::<_, Option<String>>(0));
    match value {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(BenchError::Sqlite(e.to_string())),
    }
}

fn query_first_i64(conn: &rusqlite::Connection, sql: &str) -> Result<i64, BenchError> {
    conn.query_row(sql, [], |row| row.get::<_, Option<i64>>(0))
        .map(|value| value.unwrap_or(0))
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(0),
            other => Err(BenchError::Sqlite(other.to_string())),
        })
}

fn eval_check(check: &Check, conn: &rusqlite::Connection) -> Result<(bool, String), BenchError> {
    match check.kind.as_str() {
        "sql_expect" => {
            let sql = check
                .sql
                .as_deref()
                .ok_or_else(|| BenchError::CheckConfig(check.id.clone(), "missing sql".into()))?;
            let expect = check.expect.as_deref().ok_or_else(|| {
                BenchError::CheckConfig(check.id.clone(), "missing expect".into())
            })?;
            let actual = query_first_string(conn, sql)?;
            match actual {
                Some(v) if v == expect => Ok((true, v.to_string())),
                Some(v) => Ok((false, format!("got '{v}', want '{expect}'"))),
                None => Ok((false, format!("no rows, want '{expect}'"))),
            }
        }
        "sql_nonzero" => {
            let sql = check
                .sql
                .as_deref()
                .ok_or_else(|| BenchError::CheckConfig(check.id.clone(), "missing sql".into()))?;
            let count = query_first_i64(conn, sql)?;
            Ok((count > 0, format!("count={count}")))
        }
        other => Err(BenchError::CheckConfig(
            check.id.clone(),
            format!("unknown kind '{other}'"),
        )),
    }
}

pub fn run_checks(spec: &TaskSpec, artifact: &Artifact) -> Result<Vec<CheckResult>, BenchError> {
    let conn = artifact
        .harness_db
        .as_ref()
        .map(|db| {
            rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|e| BenchError::Sqlite(e.to_string()))
        })
        .transpose()?;

    let mut results = Vec::with_capacity(spec.checks.len());
    for check in &spec.checks {
        let (passed, detail) = match conn.as_ref() {
            Some(conn) => eval_check(check, conn)?,
            None => (false, "no harness.db (bare arm)".to_string()),
        };
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
    if artifact.meta.task != spec.id {
        return Err(BenchError::TaskMismatch {
            spec: spec.id.clone(),
            artifact: artifact.meta.task.clone(),
        });
    }
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

    fn artifact_with_db(lane: &str, traces: usize) -> (tempfile::TempDir, Artifact) {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("worktree")).unwrap();
        fs::write(
            tmp.path().join("meta.json"),
            r#"{"task":"T1","arm":"hn","k":1,"agent":"fake","exit_code":0}"#,
        )
        .unwrap();
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
        let (_tmp, artifact) = artifact_with_db("tiny", 1);
        let spec = spec_with("true");
        assert!(functional_pass(&spec, &artifact).unwrap());
    }

    #[test]
    fn functional_fails_on_exit_nonzero() {
        let (_tmp, artifact) = artifact_with_db("tiny", 1);
        let spec = spec_with("false");
        assert!(!functional_pass(&spec, &artifact).unwrap());
    }

    #[test]
    fn functional_command_uses_the_platform_shell() {
        let command = functional_command("echo ok");
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        #[cfg(windows)]
        {
            assert_eq!(command.get_program(), "cmd");
            assert_eq!(args, ["/C", "echo ok"]);
        }
        #[cfg(not(windows))]
        {
            assert_eq!(command.get_program(), "sh");
            assert_eq!(args, ["-c", "echo ok"]);
        }
    }

    #[test]
    fn sql_expect_matches_and_mismatches() {
        let (_tmp, artifact) = artifact_with_db("tiny", 1);
        let results = run_checks(&spec_with("true"), &artifact).unwrap();
        assert!(results[0].passed, "lane matches 'tiny'");

        let (_tmp2, wrong) = artifact_with_db("high-risk", 1);
        let results = run_checks(&spec_with("true"), &wrong).unwrap();
        assert!(!results[0].passed, "lane 'high-risk' != 'tiny'");
    }

    #[test]
    fn sql_nonzero_reflects_trace_count() {
        let (_tmp, has) = artifact_with_db("tiny", 3);
        assert!(run_checks(&spec_with("true"), &has).unwrap()[1].passed);

        let (_tmp2, none) = artifact_with_db("tiny", 0);
        assert!(!run_checks(&spec_with("true"), &none).unwrap()[1].passed);
    }

    #[test]
    fn sql_nonzero_zero_rows_does_not_abort_run() {
        // Non-aggregate query that returns zero rows (as opposed to an
        // aggregate like `count(*)`, which always returns exactly one row).
        let (_tmp, artifact) = artifact_with_db("tiny", 1);
        let toml = r#"
id = "T1"
lane = "tiny"

[functional]
test_command = "true"

[[checks]]
id = "no_matching_rows"
responsibility = "Observability"
kind = "sql_nonzero"
sql = "SELECT id FROM trace WHERE 0"
"#;
        let spec = TaskSpec::from_toml_str(toml).unwrap();
        let score = score_artifact(&spec, &artifact).unwrap();
        assert_eq!(score.checks.len(), 1);
        assert!(
            !score.checks[0].passed,
            "zero rows should be treated as count 0, not error"
        );
    }

    #[test]
    fn sql_null_values_fail_checks_without_aborting() {
        let (_tmp, artifact) = artifact_with_db("tiny", 0);
        let toml = r#"
id = "T1"
lane = "tiny"

[functional]
test_command = "true"

[[checks]]
id = "null_string"
responsibility = "Task specification"
kind = "sql_expect"
sql = "SELECT NULL"
expect = "tiny"

[[checks]]
id = "null_integer"
responsibility = "Observability"
kind = "sql_nonzero"
sql = "SELECT NULL"
"#;
        let spec = TaskSpec::from_toml_str(toml).unwrap();
        let results = run_checks(&spec, &artifact).unwrap();

        assert_eq!(results.len(), 2);
        assert!(!results[0].passed);
        assert_eq!(results[0].detail, "no rows, want 'tiny'");
        assert!(!results[1].passed);
        assert_eq!(results[1].detail, "count=0");
    }

    #[test]
    fn unknown_check_kind_is_rejected() {
        let (_tmp, artifact) = artifact_with_db("tiny", 1);
        let toml = r#"
id = "T1"
lane = "tiny"

[functional]
test_command = "true"

[[checks]]
id = "mystery"
responsibility = "Task specification"
kind = "bogus"
"#;
        let spec = TaskSpec::from_toml_str(toml).unwrap();
        let err = run_checks(&spec, &artifact).unwrap_err();
        assert!(matches!(err, BenchError::CheckConfig(_, _)));
    }

    #[test]
    fn task_mismatch_between_artifact_and_spec_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("worktree")).unwrap();
        fs::write(
            tmp.path().join("meta.json"),
            r#"{"task":"T2","arm":"hn","k":1,"agent":"fake","exit_code":0}"#,
        )
        .unwrap();
        seed_db(&tmp.path().join("harness.db"), "tiny", 1);
        let artifact = Artifact::load(tmp.path()).unwrap();

        // spec_with() builds a spec whose id is "T1".
        let err = score_artifact(&spec_with("true"), &artifact).unwrap_err();
        assert!(matches!(
            err,
            BenchError::TaskMismatch { spec, artifact } if spec == "T1" && artifact == "T2"
        ));
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
