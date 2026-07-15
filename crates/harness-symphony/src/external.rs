use std::io::BufRead;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::config::ResolvedConfig;
use crate::run::{finalize_prepared_run, CompletedRun, PreparedRun, RunError};
use crate::run_events::{read_last_event, RunEventWriter};
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

const WINTER_NAME_POOL_SIZE: u8 = 5;
const OUTPUT_LINE_MAX_CHARS: usize = 2_000;
const OUTPUT_LEASE_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

pub fn start(
    config: &ResolvedConfig,
    run_id: &str,
    executor: Option<&str>,
) -> Result<RunRecord, ExternalError> {
    reconcile_external_runs(config)?;
    let store = RunStateStore::new(config.state_db.clone());
    let executor = match executor {
        Some(executor) => {
            validate_bounded("executor", executor, 64)?;
            executor.trim().to_owned()
        }
        None => next_winter_name(&store)?,
    };
    let record = store.show_run(run_id)?;
    if record.lightweight || !record.worktree.is_dir() {
        return Err(ExternalError::Invalid(
            "external execution requires an existing isolated worktree".to_owned(),
        ));
    }
    store.start_external(run_id, &executor, unix_timestamp())?;
    event_writer(config, run_id, &executor)?.append(
        "lifecycle",
        "agent",
        "external executor started",
    )?;
    Ok(store.show_run(run_id)?)
}

/// Stream external subagent output lines into the run's event log, refreshing
/// the executor lease while the stream is active.
pub fn output<R: BufRead>(
    config: &ResolvedConfig,
    run_id: &str,
    reader: R,
) -> Result<RunRecord, ExternalError> {
    reconcile_external_runs(config)?;
    let store = RunStateStore::new(config.state_db.clone());
    let record = store.show_run(run_id)?;
    if record.status != "running" || record.execution_mode != "external" {
        return Err(ExternalError::Invalid(format!(
            "run {run_id} is not a running external run"
        )));
    }
    let events = event_writer(config, run_id, &record.agent)?;
    store.heartbeat_external(run_id, unix_timestamp())?;
    let mut last_refresh = Instant::now();
    for line in reader.lines() {
        let line = line?;
        let message = line.trim();
        if !message.is_empty() {
            let message: String = message.chars().take(OUTPUT_LINE_MAX_CHARS).collect();
            events.append("output", "agent", message)?;
        }
        if last_refresh.elapsed() >= OUTPUT_LEASE_REFRESH_INTERVAL {
            store.heartbeat_external(run_id, unix_timestamp())?;
            last_refresh = Instant::now();
        }
    }
    store.heartbeat_external(run_id, unix_timestamp())?;
    Ok(store.show_run(run_id)?)
}

fn next_winter_name(store: &RunStateStore) -> Result<String, ExternalError> {
    let last_index = store
        .list_runs()?
        .into_iter()
        .find_map(|record| winter_index(&record.agent));
    let next = last_index.map_or(1, |index| index % WINTER_NAME_POOL_SIZE + 1);
    Ok(format!("Winter{next}"))
}

fn winter_index(agent: &str) -> Option<u8> {
    let index: u8 = agent.strip_prefix("Winter")?.parse().ok()?;
    (1..=WINTER_NAME_POOL_SIZE)
        .contains(&index)
        .then_some(index)
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
        if read_last_event(&path)?
            .as_ref()
            .map(|event| event.message.as_str())
            != Some(step)
        {
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

    fn add_prepared_run(
        store: &RunStateStore,
        config: &crate::config::ResolvedConfig,
        run_id: &str,
    ) {
        let worktree = config.worktrees_dir.join(run_id);
        fs::create_dir_all(&worktree).unwrap();
        store
            .add_run(NewRunRecord {
                run_id: run_id.to_owned(),
                story_id: format!("US-{run_id}"),
                branch: Some(format!("symphony/{run_id}")),
                worktree,
                lightweight: false,
                status: "prepared".to_owned(),
                result_path: None,
                sync_status: "not_applied".to_owned(),
                next_action: "prepared".to_owned(),
            })
            .unwrap();
    }

    #[test]
    fn output_streams_lines_as_events_and_refreshes_lease() {
        let temp = tempfile::tempdir().unwrap();
        let config = SymphonyConfig::default().resolve(temp.path());
        let store = RunStateStore::new(config.state_db.clone());
        add_prepared_run(&store, &config, "run_stream");
        let started_at = unix_timestamp() - 1;
        store
            .start_external("run_stream", "Winter1", started_at)
            .unwrap();

        let reader = std::io::Cursor::new("line one\n\n   \nline two\n");
        let record = output(&config, "run_stream", reader).unwrap();

        let page =
            crate::run_events::read_events_after(&event_path(&config, "run_stream"), None).unwrap();
        let output_events: Vec<_> = page
            .events
            .iter()
            .filter(|event| event.kind == "output")
            .collect();
        assert_eq!(output_events.len(), 2);
        assert_eq!(output_events[0].message, "line one");
        assert_eq!(output_events[1].message, "line two");
        assert!(output_events.iter().all(|event| event.agent == "Winter1"));
        assert!(record.heartbeat_at > Some(started_at));
    }

    #[test]
    fn output_rejects_runs_that_are_not_running_external() {
        let temp = tempfile::tempdir().unwrap();
        let config = SymphonyConfig::default().resolve(temp.path());
        let store = RunStateStore::new(config.state_db.clone());
        add_prepared_run(&store, &config, "run_prepared");

        let result = output(&config, "run_prepared", std::io::Cursor::new("ignored\n"));

        assert!(matches!(result, Err(ExternalError::Invalid(_))));
        assert!(!event_path(&config, "run_prepared").exists());
    }

    #[test]
    fn output_truncates_oversized_lines() {
        let temp = tempfile::tempdir().unwrap();
        let config = SymphonyConfig::default().resolve(temp.path());
        let store = RunStateStore::new(config.state_db.clone());
        add_prepared_run(&store, &config, "run_long");
        store
            .start_external("run_long", "Winter1", unix_timestamp())
            .unwrap();

        let long_line = "x".repeat(OUTPUT_LINE_MAX_CHARS + 50);
        output(&config, "run_long", std::io::Cursor::new(long_line)).unwrap();

        let page =
            crate::run_events::read_events_after(&event_path(&config, "run_long"), None).unwrap();
        let event = page
            .events
            .iter()
            .find(|event| event.kind == "output")
            .unwrap();
        assert_eq!(event.message.chars().count(), OUTPUT_LINE_MAX_CHARS);
    }

    #[test]
    fn start_rotates_winter_names_across_runs() {
        let temp = tempfile::tempdir().unwrap();
        let config = SymphonyConfig::default().resolve(temp.path());
        let store = RunStateStore::new(config.state_db.clone());
        add_prepared_run(&store, &config, "run_a");

        let first = start(&config, "run_a", None).unwrap();
        store.update_status("run_a", "completed", "done").unwrap();
        add_prepared_run(&store, &config, "run_b");
        let second = start(&config, "run_b", None).unwrap();

        assert_eq!(first.agent, "Winter1");
        assert_eq!(second.agent, "Winter2");
    }

    #[test]
    fn start_keeps_explicit_executor_name() {
        let temp = tempfile::tempdir().unwrap();
        let config = SymphonyConfig::default().resolve(temp.path());
        let store = RunStateStore::new(config.state_db.clone());
        add_prepared_run(&store, &config, "run_named");

        let record = start(&config, "run_named", Some("claude-subagent")).unwrap();

        assert_eq!(record.agent, "claude-subagent");
    }

    #[test]
    fn start_wraps_winter_rotation_after_winter5() {
        let temp = tempfile::tempdir().unwrap();
        let config = SymphonyConfig::default().resolve(temp.path());
        let store = RunStateStore::new(config.state_db.clone());
        add_prepared_run(&store, &config, "run_seed");
        start(&config, "run_seed", Some("Winter5")).unwrap();
        store
            .update_status("run_seed", "completed", "done")
            .unwrap();
        add_prepared_run(&store, &config, "run_wrap");

        let record = start(&config, "run_wrap", None).unwrap();

        assert_eq!(record.agent, "Winter1");
    }

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
