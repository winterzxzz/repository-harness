use std::path::{Path, PathBuf};
use std::process::Command;

use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

use crate::changeset::{changeset_files, changeset_id, ChangesetError};
use crate::config::ResolvedConfig;
use crate::state::{RunStateStore, StateError};

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("{0}")]
    Changeset(#[from] ChangesetError),
    #[error("{0}")]
    State(#[from] StateError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("harness-cli failed for {path}: {stderr}")]
    ApplyFailed { path: String, stderr: String },
    #[error("sync io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("git command failed: {0}")]
    GitFailed(String),
    #[error("checkout has local changes; commit, stash, or reset before syncing:\n{0}")]
    DirtyCheckout(String),
    #[error("approve the run before sync: {0}")]
    ApprovalRequired(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncChange {
    pub id: String,
    pub path: PathBuf,
    pub applied: bool,
    pub blocked: bool,
    pub operations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncResult {
    pub changes: Vec<SyncChange>,
}

pub fn sync_changesets(config: &ResolvedConfig) -> Result<SyncResult, SyncError> {
    refresh_checkout_from_upstream(config)?;
    let store = RunStateStore::new(config.state_db.clone());
    store.init()?;
    let paths = changeset_files(&config.changeset_directory)?;
    let mut changes = Vec::new();
    for path in paths {
        let id = changeset_id(&path)?;
        if run_requires_review_approval(&store, &id)? {
            changes.push(SyncChange {
                id,
                path,
                applied: false,
                blocked: true,
                operations: 0,
            });
            continue;
        }
        changes.push(apply_changeset_path(config, &store, path)?);
    }
    Ok(SyncResult { changes })
}

pub fn sync_changeset(config: &ResolvedConfig, run_id: &str) -> Result<SyncResult, SyncError> {
    let store = RunStateStore::new(config.state_db.clone());
    store.init()?;
    if run_requires_review_approval(&store, run_id)? {
        return Err(SyncError::ApprovalRequired(run_id.to_owned()));
    }
    refresh_checkout_from_upstream(config)?;
    let path = config
        .changeset_directory
        .join(format!("{run_id}.changeset.jsonl"));
    // A run that wrote no durable Harness records has no changeset; there is
    // nothing to apply, so the run must still be able to reach "synced".
    if !path.exists() {
        store.record_changeset_synced(run_id, &path, true)?;
        let _ = store.update_sync_status(run_id, "synced", "done");
        return Ok(SyncResult {
            changes: vec![SyncChange {
                id: run_id.to_owned(),
                path,
                applied: true,
                blocked: false,
                operations: 0,
            }],
        });
    }
    let change = apply_changeset_path(config, &store, path)?;
    Ok(SyncResult {
        changes: vec![change],
    })
}

pub fn refresh_checkout_from_upstream(config: &ResolvedConfig) -> Result<bool, SyncError> {
    if upstream_branch(&config.repo_root)?.is_none() {
        return Ok(false);
    }
    ensure_clean_checkout(&config.repo_root)?;
    git_command(&config.repo_root, &["pull", "--ff-only"])?;
    Ok(true)
}

pub fn unapplied_changesets(config: &ResolvedConfig) -> Result<Vec<PathBuf>, SyncError> {
    let store = RunStateStore::new(config.state_db.clone());
    store.init()?;
    let mut unapplied = Vec::new();
    for path in changeset_files(&config.changeset_directory)? {
        let id = changeset_id(&path)?;
        if !harness_db_has_changeset(&config.harness_db, &id)? || !store.changeset_synced(&id)? {
            unapplied.push(path);
        }
    }
    Ok(unapplied)
}

fn harness_db_has_changeset(db_path: &Path, id: &str) -> Result<bool, SyncError> {
    if !db_path.exists() {
        return Ok(false);
    }
    let connection = Connection::open(db_path)?;
    let has_changeset_table = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='changeset_applied';",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !has_changeset_table {
        return Ok(false);
    }
    connection
        .query_row(
            "SELECT 1 FROM changeset_applied WHERE id=?1;",
            params![id],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(SyncError::from)
}

fn apply_changeset_path(
    config: &ResolvedConfig,
    store: &RunStateStore,
    path: PathBuf,
) -> Result<SyncChange, SyncError> {
    let id = changeset_id(&path)?;
    if harness_db_has_changeset(&config.harness_db, &id)? && store.changeset_synced(&id)? {
        store.record_changeset_synced(&id, &path, true)?;
        let _ = store.update_sync_status(&id, "synced", "done");
        return Ok(SyncChange {
            id,
            path,
            applied: true,
            blocked: false,
            operations: 0,
        });
    }
    let output = run_changeset_apply(config, &path, true)?;
    let output = if changeset_apply_rejected_json_flag(&output) {
        // Older released harness-cli binaries predate --json; fall back to
        // the legacy text output rather than failing every sync.
        run_changeset_apply(config, &path, false)?
    } else {
        output
    };
    if !output.status.success() {
        return Err(SyncError::ApplyFailed {
            path: path.display().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = parse_apply_result(&stdout)
        .or_else(|| parse_legacy_apply_result(&stdout))
        .ok_or_else(|| SyncError::ApplyFailed {
            path: path.display().to_string(),
            stderr: format!("harness-cli did not print a recognizable apply result: {stdout}"),
        })?;
    let durable_applied = result.applied || harness_db_has_changeset(&config.harness_db, &id)?;
    let operations = result.operations;
    store.record_changeset_synced(&id, &path, durable_applied)?;
    if durable_applied {
        let _ = store.update_sync_status(&id, "synced", "done");
    }
    Ok(SyncChange {
        id,
        path,
        applied: durable_applied,
        blocked: false,
        operations,
    })
}

pub fn run_requires_review_approval(
    store: &RunStateStore,
    run_id: &str,
) -> Result<bool, SyncError> {
    match store.show_run(run_id) {
        Ok(run) => {
            Ok(run.status == "completed" && run.pr_status != "merged" && run.reviewed_at.is_none())
        }
        Err(StateError::RunNotFound(_)) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn upstream_branch(repo_root: &Path) -> Result<Option<String>, SyncError> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .current_dir(repo_root)
        .output()?;
    if output.status.success() {
        let upstream = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        return Ok((!upstream.is_empty()).then_some(upstream));
    }
    Ok(None)
}

fn ensure_clean_checkout(repo_root: &Path) -> Result<(), SyncError> {
    let output = git_output(
        repo_root,
        &["status", "--porcelain", "--untracked-files=all"],
    )?;
    let status = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_ignorable_checkout_status(line))
        .collect::<Vec<_>>()
        .join("\n");
    if status.is_empty() {
        Ok(())
    } else {
        Err(SyncError::DirtyCheckout(status))
    }
}

fn is_ignorable_checkout_status(line: &str) -> bool {
    let path = porcelain_path(line);
    path == ".harness/symphony.yml"
        || path.starts_with(".harness/runs/")
        || path.ends_with(".tsbuildinfo")
}

fn porcelain_path(line: &str) -> &str {
    line.get(3..).unwrap_or(line).trim()
}

fn git_command(repo_root: &Path, args: &[&str]) -> Result<(), SyncError> {
    let output = git_output(repo_root, args)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(SyncError::GitFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

fn git_output(repo_root: &Path, args: &[&str]) -> Result<std::process::Output, SyncError> {
    Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .map_err(SyncError::from)
}

#[derive(Debug, serde::Deserialize, PartialEq, Eq)]
struct ApplyResult {
    applied: bool,
    #[serde(default)]
    operations: usize,
}

fn run_changeset_apply(
    config: &ResolvedConfig,
    path: &Path,
    json: bool,
) -> Result<std::process::Output, SyncError> {
    let mut command = Command::new(config.repo_root.join("scripts/bin/harness-cli"));
    command
        .args(["db", "changeset", "apply"])
        .arg(path)
        .env("HARNESS_DB_PATH", &config.harness_db)
        .current_dir(&config.repo_root);
    if json {
        command.arg("--json");
    }
    command.output().map_err(SyncError::from)
}

fn changeset_apply_rejected_json_flag(output: &std::process::Output) -> bool {
    !output.status.success() && String::from_utf8_lossy(&output.stderr).contains("--json")
}

fn parse_apply_result(stdout: &str) -> Option<ApplyResult> {
    stdout
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| line.starts_with('{'))
        .and_then(|line| serde_json::from_str(line).ok())
}

// Output shape of harness-cli releases that predate --json:
// "Changeset <id> applied (N operation(s))." / "... already applied; skipped."
fn parse_legacy_apply_result(stdout: &str) -> Option<ApplyResult> {
    if stdout.contains(" applied ") {
        let operations = stdout
            .split('(')
            .nth(1)
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse::<usize>().ok())?;
        Some(ApplyResult {
            applied: true,
            operations,
        })
    } else if stdout.contains("already applied") {
        Some(ApplyResult {
            applied: false,
            operations: 0,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedConfig;
    use std::fs;
    use std::process::Command;

    #[test]
    fn parses_json_apply_result_from_cli_output() {
        assert_eq!(
            parse_apply_result(r#"{"id":"run_1","applied":true,"operations":3}"#),
            Some(ApplyResult {
                applied: true,
                operations: 3
            })
        );
        assert_eq!(
            parse_apply_result(
                "warning: noise\n{\"id\":\"run_1\",\"applied\":false,\"operations\":0}\n"
            ),
            Some(ApplyResult {
                applied: false,
                operations: 0
            })
        );
        assert_eq!(
            parse_apply_result("Changeset run_1 applied (3 operation(s))."),
            None
        );
    }

    #[test]
    fn missing_changeset_applied_table_is_treated_as_unapplied() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("harness.db");
        Connection::open(&db_path).unwrap();

        let has_changeset = harness_db_has_changeset(&db_path, "run_old").unwrap();

        assert!(!has_changeset);
    }

    #[test]
    fn bulk_sync_blocks_completed_pr_less_run_until_approved() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        fs::create_dir_all(&config.changeset_directory).unwrap();
        fs::write(
            config
                .changeset_directory
                .join("run_unapproved.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_unapproved"}
{"op":"story.update","version":1,"id":"US-REVIEW","payload":{"status":"implemented"}}
"#,
        )
        .unwrap();
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_unapproved".to_owned(),
                story_id: "US-REVIEW".to_owned(),
                branch: Some("symphony/run_unapproved".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_unapproved/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "approve or request changes".to_owned(),
            })
            .unwrap();

        let result = sync_changesets(&config).unwrap();

        assert_eq!(result.changes.len(), 1);
        assert!(result.changes[0].blocked);
        assert!(!result.changes[0].applied);
        assert!(!store.changeset_synced("run_unapproved").unwrap());
    }

    #[test]
    fn bulk_sync_blocks_unmerged_pr_run_until_reviewed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        fs::create_dir_all(&config.changeset_directory).unwrap();
        fs::write(
            config
                .changeset_directory
                .join("run_pr_review.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_pr_review"}"#,
        )
        .unwrap();
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_pr_review".to_owned(),
                story_id: "US-PR-REVIEW".to_owned(),
                branch: Some("symphony/run_pr_review".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: None,
                sync_status: "not_applied".to_owned(),
                next_action: "review pull request".to_owned(),
            })
            .unwrap();
        store
            .update_pr_url("run_pr_review", "https://example.test/pr/1")
            .unwrap();

        let result = sync_changesets(&config).unwrap();

        assert_eq!(result.changes.len(), 1);
        assert!(result.changes[0].blocked);
        assert!(!store.changeset_synced("run_pr_review").unwrap());
    }

    #[test]
    fn already_applied_changeset_marks_run_synced() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        fs::create_dir_all(&config.changeset_directory).unwrap();
        fs::create_dir_all(temp_dir.path().join("scripts/bin")).unwrap();
        fs::write(
            config.changeset_directory.join("run_done.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_done"}
{"op":"story.update","version":1,"id":"US-DONE","payload":{"status":"implemented"}}
"#,
        )
        .unwrap();
        let connection = Connection::open(&config.harness_db).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE changeset_applied (
                    id TEXT PRIMARY KEY,
                    path TEXT,
                    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                INSERT INTO changeset_applied (id, path)
                VALUES ('run_done', '.harness/changesets/run_done.changeset.jsonl');",
            )
            .unwrap();
        let cli_path = temp_dir.path().join("scripts/bin/harness-cli");
        fs::write(
            &cli_path,
            "#!/bin/sh\necho '{\"id\":\"run_done\",\"applied\":false,\"operations\":0}'\n",
        )
        .unwrap();
        make_executable(&cli_path);
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_done".to_owned(),
                story_id: "US-DONE".to_owned(),
                branch: Some("symphony/run_done".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_done/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "approve sync".to_owned(),
            })
            .unwrap();
        store.approve_run("run_done", "test approval").unwrap();

        let result = sync_changeset(&config, "run_done").unwrap();
        let run = store.show_run("run_done").unwrap();

        assert!(result.changes[0].applied);
        assert_eq!(result.changes[0].operations, 0);
        assert!(store.changeset_synced("run_done").unwrap());
        assert_eq!(run.sync_status, "synced");
        assert_eq!(run.next_action, "done");
    }

    #[test]
    fn already_synced_changeset_heals_unsynced_run_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        fs::create_dir_all(&config.changeset_directory).unwrap();
        fs::write(
            config.changeset_directory.join("run_heal.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_heal"}
{"op":"story.update","version":1,"id":"US-HEAL","payload":{"status":"implemented"}}
"#,
        )
        .unwrap();
        let connection = Connection::open(&config.harness_db).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE changeset_applied (
                    id TEXT PRIMARY KEY,
                    path TEXT,
                    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                INSERT INTO changeset_applied (id, path)
                VALUES ('run_heal', '.harness/changesets/run_heal.changeset.jsonl');",
            )
            .unwrap();
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_heal".to_owned(),
                story_id: "US-HEAL".to_owned(),
                branch: Some("symphony/run_heal".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_heal/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "approve sync".to_owned(),
            })
            .unwrap();
        store
            .record_changeset_synced(
                "run_heal",
                std::path::Path::new(".harness/changesets/run_heal.changeset.jsonl"),
                false,
            )
            .unwrap();
        store.approve_run("run_heal", "test approval").unwrap();

        let result = sync_changeset(&config, "run_heal").unwrap();
        let run = store.show_run("run_heal").unwrap();

        assert!(result.changes[0].applied);
        assert_eq!(result.changes[0].operations, 0);
        assert_eq!(run.sync_status, "synced");
        assert_eq!(run.next_action, "done");
    }

    #[test]
    fn sync_changeset_without_changeset_file_marks_run_synced() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        fs::create_dir_all(&config.changeset_directory).unwrap();
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_docs_only".to_owned(),
                story_id: "US-DOCS".to_owned(),
                branch: Some("symphony/run_docs_only".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_docs_only/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "approve sync".to_owned(),
            })
            .unwrap();
        store.approve_run("run_docs_only", "test approval").unwrap();

        let result = sync_changeset(&config, "run_docs_only").unwrap();
        let run = store.show_run("run_docs_only").unwrap();

        assert!(result.changes[0].applied);
        assert_eq!(result.changes[0].operations, 0);
        assert_eq!(run.sync_status, "synced");
        assert_eq!(run.next_action, "done");
    }

    #[test]
    fn legacy_apply_output_is_parsed_when_json_is_unavailable() {
        assert_eq!(
            parse_legacy_apply_result("Changeset run_1 applied (3 operation(s))."),
            Some(ApplyResult {
                applied: true,
                operations: 3
            })
        );
        assert_eq!(
            parse_legacy_apply_result("Changeset run_1 already applied; skipped."),
            Some(ApplyResult {
                applied: false,
                operations: 0
            })
        );
        assert_eq!(parse_legacy_apply_result("unrelated output"), None);
    }

    #[test]
    fn sync_changeset_applies_only_requested_run() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        fs::create_dir_all(&config.changeset_directory).unwrap();
        fs::create_dir_all(temp_dir.path().join("scripts/bin")).unwrap();
        fs::write(
            config.changeset_directory.join("run_one.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_one"}
{"op":"story.update","version":1,"id":"US-ONE","payload":{"status":"implemented"}}
"#,
        )
        .unwrap();
        fs::write(
            config.changeset_directory.join("run_two.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_two"}
{"op":"story.update","version":1,"id":"US-TWO","payload":{"status":"implemented"}}
"#,
        )
        .unwrap();
        let cli_path = temp_dir.path().join("scripts/bin/harness-cli");
        fs::write(
            &cli_path,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> sync-args.log\necho '{\"id\":\"run_one\",\"applied\":true,\"operations\":2}'\n",
        )
        .unwrap();
        make_executable(&cli_path);

        let result = sync_changeset(&config, "run_one").unwrap();

        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].id, "run_one");
        assert!(result.changes[0].applied);
        let args = fs::read_to_string(temp_dir.path().join("sync-args.log")).unwrap();
        assert!(args.contains(".harness/changesets/run_one.changeset.jsonl"));
        assert!(!args.contains("run_two.changeset.jsonl"));
    }

    #[test]
    fn refresh_checkout_fast_forwards_from_upstream() {
        let temp_dir = tempfile::tempdir().unwrap();
        let remote = temp_dir.path().join("remote.git");
        run_git(
            temp_dir.path(),
            &["init", "--bare", &remote.display().to_string()],
        );
        let local = temp_dir.path().join("local");
        let other = temp_dir.path().join("other");
        run_git(
            temp_dir.path(),
            &[
                "clone",
                &remote.display().to_string(),
                &local.display().to_string(),
            ],
        );
        configure_git(&local);
        fs::write(local.join("README.md"), "one\n").unwrap();
        run_git(&local, &["add", "README.md"]);
        run_git(&local, &["commit", "-m", "one"]);
        run_git(&local, &["push", "-u", "origin", "HEAD"]);
        run_git(
            temp_dir.path(),
            &[
                "clone",
                &remote.display().to_string(),
                &other.display().to_string(),
            ],
        );
        configure_git(&other);
        fs::write(other.join("README.md"), "two\n").unwrap();
        run_git(&other, &["commit", "-am", "two"]);
        run_git(&other, &["push"]);

        let refreshed = refresh_checkout_from_upstream(&config_for_root(&local)).unwrap();

        assert!(refreshed);
        assert_eq!(
            fs::read_to_string(local.join("README.md")).unwrap(),
            "two\n"
        );
    }

    #[test]
    fn refresh_checkout_refuses_dirty_checkout() {
        let temp_dir = tempfile::tempdir().unwrap();
        let remote = temp_dir.path().join("remote.git");
        run_git(
            temp_dir.path(),
            &["init", "--bare", &remote.display().to_string()],
        );
        let local = temp_dir.path().join("local");
        run_git(
            temp_dir.path(),
            &[
                "clone",
                &remote.display().to_string(),
                &local.display().to_string(),
            ],
        );
        configure_git(&local);
        fs::write(local.join("README.md"), "one\n").unwrap();
        run_git(&local, &["add", "README.md"]);
        run_git(&local, &["commit", "-m", "one"]);
        run_git(&local, &["push", "-u", "origin", "HEAD"]);
        fs::write(local.join("local.txt"), "dirty\n").unwrap();

        let error = refresh_checkout_from_upstream(&config_for_root(&local)).unwrap_err();

        assert!(matches!(error, SyncError::DirtyCheckout(status) if status.contains("local.txt")));
    }

    #[test]
    fn refresh_checkout_allows_only_local_symphony_artifacts() {
        let temp_dir = tempfile::tempdir().unwrap();
        let remote = temp_dir.path().join("remote.git");
        run_git(
            temp_dir.path(),
            &["init", "--bare", &remote.display().to_string()],
        );
        let local = temp_dir.path().join("local");
        run_git(
            temp_dir.path(),
            &[
                "clone",
                &remote.display().to_string(),
                &local.display().to_string(),
            ],
        );
        configure_git(&local);
        fs::write(local.join("README.md"), "one\n").unwrap();
        run_git(&local, &["add", "README.md"]);
        run_git(&local, &["commit", "-m", "one"]);
        run_git(&local, &["push", "-u", "origin", "HEAD"]);
        fs::create_dir_all(local.join(".harness/runs/run_1")).unwrap();
        fs::write(local.join(".harness/runs/run_1/RESULT.json"), "{}\n").unwrap();
        fs::write(local.join(".harness/symphony.yml"), "version: 1\n").unwrap();

        let refreshed = refresh_checkout_from_upstream(&config_for_root(&local)).unwrap();

        assert!(refreshed);
    }

    #[test]
    fn refresh_checkout_allows_generated_typescript_build_info() {
        let temp_dir = tempfile::tempdir().unwrap();
        let remote = temp_dir.path().join("remote.git");
        run_git(
            temp_dir.path(),
            &["init", "--bare", &remote.display().to_string()],
        );
        let local = temp_dir.path().join("local");
        run_git(
            temp_dir.path(),
            &[
                "clone",
                &remote.display().to_string(),
                &local.display().to_string(),
            ],
        );
        configure_git(&local);
        fs::write(local.join("README.md"), "one\n").unwrap();
        run_git(&local, &["add", "README.md"]);
        run_git(&local, &["commit", "-m", "one"]);
        run_git(&local, &["push", "-u", "origin", "HEAD"]);
        fs::create_dir_all(local.join("crates/harness-symphony/web-ui")).unwrap();
        fs::write(
            local.join("crates/harness-symphony/web-ui/tsconfig.tsbuildinfo"),
            "{}\n",
        )
        .unwrap();

        let refreshed = refresh_checkout_from_upstream(&config_for_root(&local)).unwrap();

        assert!(refreshed);
    }

    #[test]
    fn refresh_checkout_still_refuses_code_changes_with_local_symphony_artifacts() {
        let temp_dir = tempfile::tempdir().unwrap();
        let remote = temp_dir.path().join("remote.git");
        run_git(
            temp_dir.path(),
            &["init", "--bare", &remote.display().to_string()],
        );
        let local = temp_dir.path().join("local");
        run_git(
            temp_dir.path(),
            &[
                "clone",
                &remote.display().to_string(),
                &local.display().to_string(),
            ],
        );
        configure_git(&local);
        fs::write(local.join("README.md"), "one\n").unwrap();
        run_git(&local, &["add", "README.md"]);
        run_git(&local, &["commit", "-m", "one"]);
        run_git(&local, &["push", "-u", "origin", "HEAD"]);
        fs::create_dir_all(local.join(".harness/runs/run_1")).unwrap();
        fs::write(local.join(".harness/runs/run_1/RESULT.json"), "{}\n").unwrap();
        fs::write(local.join(".harness/symphony.yml"), "version: 1\n").unwrap();
        fs::write(local.join("local.txt"), "dirty\n").unwrap();

        let error = refresh_checkout_from_upstream(&config_for_root(&local)).unwrap_err();

        assert!(
            matches!(error, SyncError::DirtyCheckout(status) if status.contains("local.txt") && !status.contains(".harness/runs") && !status.contains(".harness/symphony.yml"))
        );
    }

    #[test]
    fn refresh_checkout_refuses_unapplied_harness_changesets() {
        let temp_dir = tempfile::tempdir().unwrap();
        let remote = temp_dir.path().join("remote.git");
        run_git(
            temp_dir.path(),
            &["init", "--bare", &remote.display().to_string()],
        );
        let local = temp_dir.path().join("local");
        run_git(
            temp_dir.path(),
            &[
                "clone",
                &remote.display().to_string(),
                &local.display().to_string(),
            ],
        );
        configure_git(&local);
        fs::write(local.join("README.md"), "one\n").unwrap();
        run_git(&local, &["add", "README.md"]);
        run_git(&local, &["commit", "-m", "one"]);
        run_git(&local, &["push", "-u", "origin", "HEAD"]);
        fs::create_dir_all(local.join(".harness/changesets")).unwrap();
        fs::write(
            local.join(".harness/changesets/run_1.changeset.jsonl"),
            "{}\n",
        )
        .unwrap();

        let error = refresh_checkout_from_upstream(&config_for_root(&local)).unwrap_err();

        assert!(
            matches!(error, SyncError::DirtyCheckout(status) if status.contains(".harness/changesets"))
        );
    }

    fn configure_git(repo: &Path) {
        run_git(repo, &["config", "user.email", "test@example.invalid"]);
        run_git(repo, &["config", "user.name", "Test User"]);
    }

    fn run_git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}

    fn config_for_root(root: &Path) -> ResolvedConfig {
        ResolvedConfig {
            version: 1,
            repo_root: root.to_path_buf(),
            harness_db: root.join("harness.db"),
            state_db: root.join(".symphony/state.db"),
            runs_dir: root.join(".harness/runs"),
            worktrees_dir: root.join(".symphony/worktrees"),
            single_active_run: true,
            agent_adapter: "custom".to_owned(),
            agent_command: vec![],
            agent_timeout_minutes: 120,
            pull_request_create: "ask".to_owned(),
            pull_request_provider: "github".to_owned(),
            pull_request_draft_for: vec![],
            changeset_directory: root.join(".harness/changesets"),
            changeset_render_in_summary: true,
            allow_here_for_tiny: true,
            compact_keep_last: 50,
            external_heartbeat_ttl_seconds: 120,
            keep_failed_worktrees: true,
            cleanup_after_sync: false,
            failed_worktree_retention_days: 7,
            auto_source: "harness-db".to_owned(),
            auto_poll_interval_seconds: 30,
            auto_max_attempts: 3,
            auto_allow_stale_base: false,
            e2e_timeout_minutes: 15,
        }
    }
}
