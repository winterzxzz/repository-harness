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

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("functional: PASS"), "stdout was:\n{stdout}");
    assert!(
        stdout.contains("[PASS] intake_lane"),
        "stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("[PASS] trace_recorded"),
        "stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("Task specification"),
        "stdout was:\n{stdout}"
    );
    assert!(stdout.contains("1/1"), "stdout was:\n{stdout}");
}
