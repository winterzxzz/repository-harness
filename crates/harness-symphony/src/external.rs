use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::config::ResolvedConfig;
use crate::run::{finalize_prepared_run, CompletedRun, PreparedRun, RunError};
use crate::run_events::{read_events_after, RunEventWriter};
use crate::state::{RunRecord, RunStateStore, StateError};

#[derive(Debug, Error)]
pub enum ExternalError {
    #[error("{0}")]
    State(#[from] StateError),
    #[error("{0}")]
    Run(#[from] RunError),
    #[error("external executor event error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid external executor request: {0}")]
    Invalid(String),
}

pub fn reconcile_external_runs(config: &ResolvedConfig) -> Result<Vec<String>, ExternalError> {
    Ok(RunStateStore::new(config.state_db.clone())
        .reconcile_expired_external_runs(unix_timestamp(), config.external_heartbeat_ttl_seconds)?)
}

pub fn start(
    config: &ResolvedConfig,
    run_id: &str,
    executor: &str,
) -> Result<RunRecord, ExternalError> {
    reconcile_external_runs(config)?;
    validate_bounded("executor", executor, 64)?;
    let store = RunStateStore::new(config.state_db.clone());
    let record = store.show_run(run_id)?;
    if record.lightweight || !record.worktree.is_dir() {
        return Err(ExternalError::Invalid(
            "external execution requires an existing isolated worktree".to_owned(),
        ));
    }
    store.start_external(run_id, executor.trim(), unix_timestamp())?;
    event_writer(config, run_id, executor.trim())?.append(
        "lifecycle",
        "agent",
        "external executor started",
    )?;
    Ok(store.show_run(run_id)?)
}

pub fn heartbeat(
    config: &ResolvedConfig,
    run_id: &str,
    step: Option<&str>,
) -> Result<RunRecord, ExternalError> {
    reconcile_external_runs(config)?;
    if let Some(step) = step {
        validate_bounded("step", step, 200)?;
    }
    let store = RunStateStore::new(config.state_db.clone());
    let record = store.show_run(run_id)?;
    store.heartbeat_external(run_id, unix_timestamp())?;
    if let Some(step) = step.map(str::trim) {
        let path = event_path(config, run_id);
        let page = read_events_after(&path, None)?;
        if page.events.last().map(|event| event.message.as_str()) != Some(step) {
            RunEventWriter::new(path, &record.agent)?.append("progress", "agent", step)?;
        }
    }
    Ok(store.show_run(run_id)?)
}

pub fn complete(config: &ResolvedConfig, run_id: &str) -> Result<CompletedRun, ExternalError> {
    reconcile_external_runs(config)?;
    let store = RunStateStore::new(config.state_db.clone());
    let record = store.show_run(run_id)?;
    if record.lightweight
        || record.execution_mode != "external"
        || !matches!(record.status.as_str(), "running" | "stale")
    {
        return Err(ExternalError::Invalid(format!(
            "run {run_id} is not a running or stale external run"
        )));
    }
    let prepared = PreparedRun {
        run_id: record.run_id,
        story_id: record.story_id,
        branch: record.branch,
        harness_db_path: record.worktree.join("harness.db"),
        contract_path: config.runs_dir.join(run_id).join("RUN_CONTRACT.json"),
        worktree: record.worktree,
        lightweight: false,
        request_changes: None,
    };
    Ok(finalize_prepared_run(config, prepared)?)
}

fn validate_bounded(name: &str, value: &str, maximum: usize) -> Result<(), ExternalError> {
    let length = value.trim().chars().count();
    if length == 0 || length > maximum {
        return Err(ExternalError::Invalid(format!(
            "{name} must be 1-{maximum} characters"
        )));
    }
    Ok(())
}

fn event_path(config: &ResolvedConfig, run_id: &str) -> std::path::PathBuf {
    config.runs_dir.join(run_id).join("RUN_EVENTS.jsonl")
}

fn event_writer(
    config: &ResolvedConfig,
    run_id: &str,
    executor: &str,
) -> Result<RunEventWriter, ExternalError> {
    Ok(RunEventWriter::new(event_path(config, run_id), executor)?)
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use crate::config::SymphonyConfig;
    use crate::harness_digest::logical_digest;
    use crate::state::NewRunRecord;

    use super::*;

    #[test]
    fn bounded_values_reject_blank_and_oversized_input() {
        assert!(validate_bounded("step", "tests passing", 200).is_ok());
        assert!(validate_bounded("step", "   ", 200).is_err());
        assert!(validate_bounded("step", &"x".repeat(201), 200).is_err());
    }

    #[test]
    fn stale_run_can_complete_late_without_releasing_newer_active_lock() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = SymphonyConfig::default().resolve(temp.path());
        config.external_heartbeat_ttl_seconds = 1;
        let old_worktree = config.worktrees_dir.join("run_old");
        fs::create_dir_all(old_worktree.join(".harness/runs/run_old")).unwrap();
        assert!(Command::new("git")
            .arg("init")
            .current_dir(&old_worktree)
            .status()
            .unwrap()
            .success());
        let connection = rusqlite::Connection::open(old_worktree.join("harness.db")).unwrap();
        connection
            .execute("CREATE TABLE fixture (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        drop(connection);
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(NewRunRecord {
                run_id: "run_old".to_owned(),
                story_id: "US-OLD".to_owned(),
                branch: Some("symphony/run_old".to_owned()),
                worktree: old_worktree.clone(),
                lightweight: false,
                status: "prepared".to_owned(),
                result_path: None,
                sync_status: "not_applied".to_owned(),
                next_action: "prepared".to_owned(),
            })
            .unwrap();
        store
            .set_harness_db_digest(
                "run_old",
                &logical_digest(&old_worktree.join("harness.db")).unwrap(),
            )
            .unwrap();
        store.start_external("run_old", "executor", 1).unwrap();
        assert_eq!(
            store.reconcile_expired_external_runs(2, 1).unwrap(),
            vec!["run_old"]
        );
        store
            .add_run(NewRunRecord {
                run_id: "run_new".to_owned(),
                story_id: "US-NEW".to_owned(),
                branch: Some("symphony/run_new".to_owned()),
                worktree: config.worktrees_dir.join("run_new"),
                lightweight: false,
                status: "prepared".to_owned(),
                result_path: None,
                sync_status: "not_applied".to_owned(),
                next_action: "prepared".to_owned(),
            })
            .unwrap();
        fs::write(
            old_worktree.join(".harness/runs/run_old/SUMMARY.md"),
            "# Late completion\n",
        )
        .unwrap();
        fs::write(
            old_worktree.join(".harness/runs/run_old/RESULT.json"),
            r#"{"version":1,"run_id":"run_old","story_id":"US-OLD","outcome":"completed","validation":{"commands":[{"command":"true","result":"pass"}]}}"#,
        )
        .unwrap();

        let completed = complete(&config, "run_old").unwrap();

        assert_eq!(completed.outcome, "completed");
        assert_eq!(store.show_run("run_old").unwrap().status, "completed");
        assert_eq!(store.active_run().unwrap().unwrap().run_id, "run_new");
    }
}
