use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use thiserror::Error;

use crate::config::ResolvedConfig;
use crate::state::{RunStateStore, StateError};

#[derive(Debug, Error)]
pub enum RetentionError {
    #[error("--keep-last must be at least 1")]
    UnsafeKeepLast,
    #[error("retention io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    State(#[from] StateError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactResult {
    pub kept: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
}

pub fn compact_runs(
    config: &ResolvedConfig,
    keep_last: u32,
) -> Result<CompactResult, RetentionError> {
    if keep_last == 0 {
        return Err(RetentionError::UnsafeKeepLast);
    }
    let terminal = RunStateStore::new(config.state_db.clone())
        .list_cleanup_runs()?
        .into_iter()
        .filter(|run| !matches!(run.status.as_str(), "prepared" | "running"))
        .map(|run| run.run_id)
        .collect::<HashSet<_>>();
    let mut runs = run_dirs(config, &terminal)?;
    runs.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));

    let keep = keep_last as usize;
    let mut kept = Vec::new();
    let mut removed = Vec::new();
    for (index, (_, path)) in runs.into_iter().enumerate() {
        if index < keep {
            kept.push(path);
        } else {
            fs::remove_dir_all(&path)?;
            removed.push(path);
        }
    }
    Ok(CompactResult { kept, removed })
}

fn run_dirs(
    config: &ResolvedConfig,
    terminal: &HashSet<String>,
) -> Result<Vec<(std::time::SystemTime, PathBuf)>, RetentionError> {
    let mut runs = Vec::new();
    if !config.runs_dir.exists() {
        return Ok(runs);
    }
    for entry in fs::read_dir(&config.runs_dir)? {
        let entry = entry?;
        let path = entry.path();
        let run_id = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() && terminal.contains(&run_id) {
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            runs.push((modified, path));
        }
    }
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedConfig;
    use crate::state::{NewRunRecord, RunStateStore};

    fn config(root: &std::path::Path) -> ResolvedConfig {
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
        }
    }

    fn add_run(config: &ResolvedConfig, id: &str, status: &str) {
        RunStateStore::new(config.state_db.clone())
            .add_run(NewRunRecord {
                run_id: id.to_owned(),
                story_id: "US-092".to_owned(),
                branch: None,
                worktree: config.worktrees_dir.join(id),
                lightweight: false,
                status: status.to_owned(),
                result_path: None,
                sync_status: "not_applied".to_owned(),
                next_action: "review".to_owned(),
            })
            .unwrap();
    }

    #[test]
    fn compacts_run_dirs_without_touching_changesets() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config(temp_dir.path());
        fs::create_dir_all(config.runs_dir.join("run_1")).unwrap();
        fs::create_dir_all(config.runs_dir.join("run_2")).unwrap();
        add_run(&config, "run_1", "completed");
        add_run(&config, "run_2", "completed");
        fs::create_dir_all(config.changeset_directory.clone()).unwrap();
        fs::write(
            config.changeset_directory.join("run_1.changeset.jsonl"),
            "{}",
        )
        .unwrap();

        let result = compact_runs(&config, 1).unwrap();

        assert_eq!(result.kept.len(), 1);
        assert_eq!(result.removed.len(), 1);
        assert!(config
            .changeset_directory
            .join("run_1.changeset.jsonl")
            .exists());
    }

    #[test]
    fn preserves_active_and_unknown_run_evidence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config(temp_dir.path());
        for id in ["run_active", "run_terminal", "run_unknown"] {
            fs::create_dir_all(config.runs_dir.join(id)).unwrap();
        }
        add_run(&config, "run_active", "prepared");
        RunStateStore::new(config.state_db.clone())
            .update_status("run_active", "completed", "review")
            .unwrap();
        add_run(&config, "run_terminal", "completed");
        RunStateStore::new(config.state_db.clone())
            .update_status("run_active", "running", "work")
            .unwrap();

        compact_runs(&config, 1).unwrap();

        assert!(config.runs_dir.join("run_active").exists());
        assert!(config.runs_dir.join("run_unknown").exists());
        assert!(config.runs_dir.join("run_terminal").exists());
    }

    #[test]
    fn refuses_zero_keep_last() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config(temp_dir.path());

        assert!(matches!(
            compact_runs(&config, 0).unwrap_err(),
            RetentionError::UnsafeKeepLast
        ));
    }
}
