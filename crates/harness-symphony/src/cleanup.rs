use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::config::ResolvedConfig;
use crate::state::{CleanupRunRecord, RunStateStore, StateError};

#[derive(Debug, Error)]
pub enum CleanupError {
    #[error("{0}")]
    State(#[from] StateError),
    #[error("cleanup io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsafe cleanup path: {0}")]
    UnsafePath(PathBuf),
    #[error("cleanup completed with {0} deletion failure(s)")]
    DeletionFailures(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupReason {
    Done,
    ExpiredFailed,
    ExpiredInterrupted,
    ExpiredCancelled,
    ExpiredStale,
    Orphan,
}

impl std::fmt::Display for CleanupReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Done => "done",
            Self::ExpiredFailed => "expired-failed",
            Self::ExpiredInterrupted => "expired-interrupted",
            Self::ExpiredCancelled => "expired-cancelled",
            Self::ExpiredStale => "expired-stale",
            Self::Orphan => "orphan",
        })
    }
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
    pub fn removed_count(&self) -> usize {
        self.items.iter().filter(|item| item.removed).count()
    }
    pub fn failures(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.error.is_some())
            .count()
    }
    pub fn reclaimed_bytes(&self) -> u64 {
        self.items.iter().map(|item| item.reclaimed_bytes).sum()
    }
}

pub fn cleanup_runtime(
    config: &ResolvedConfig,
    dry_run: bool,
) -> Result<CleanupResult, CleanupError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    cleanup_runtime_at(config, dry_run, now)
}

fn cleanup_runtime_at(
    config: &ResolvedConfig,
    dry_run: bool,
    now: i64,
) -> Result<CleanupResult, CleanupError> {
    let store = RunStateStore::new(config.state_db.clone());
    store.reconcile_expired_external_runs(now, config.external_heartbeat_ttl_seconds)?;
    let runs = store.list_cleanup_runs()?;
    let registered = registered_worktrees(config)?;
    let known = runs
        .iter()
        .map(|run| path_key(&absolute_worktree(config, &run.worktree)))
        .collect::<HashSet<_>>();
    let active = runs
        .iter()
        .filter(|run| matches!(run.status.as_str(), "prepared" | "running"))
        .map(|run| path_key(&absolute_worktree(config, &run.worktree)))
        .collect::<HashSet<_>>();
    let mut candidates = runs
        .iter()
        .filter_map(|run| {
            let path = absolute_worktree(config, &run.worktree);
            (!active.contains(&path_key(&path)))
                .then(|| eligible_reason(run, config, now))
                .flatten()
                .map(|reason| (Some(run.run_id.clone()), path, reason))
        })
        .collect::<Vec<_>>();
    candidates.extend(orphan_candidates(config, &known, &active, now)?);
    candidates.sort_by(|left, right| left.1.cmp(&right.1));
    candidates.dedup_by(|left, right| left.1 == right.1);

    let mut result = CleanupResult::default();
    for (run_id, path, reason) in candidates {
        if !path.exists() {
            continue;
        }
        let mut item = CleanupItem {
            run_id,
            path: path.clone(),
            reason,
            removed: false,
            reclaimed_bytes: 0,
            error: None,
        };
        match safe_candidate(&config.worktrees_dir, &path) {
            Ok(safe) => {
                item.reclaimed_bytes = directory_size(&safe).unwrap_or(0);
                if !dry_run {
                    match remove_worktree(config, &safe, registered.contains(&safe)) {
                        Ok(()) => item.removed = true,
                        Err(error) => item.error = Some(error.to_string()),
                    }
                }
            }
            Err(error) => item.error = Some(error.to_string()),
        }
        result.items.push(item);
    }
    if !dry_run {
        let _ = Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(&config.repo_root)
            .output();
    }
    Ok(result)
}

fn eligible_reason(
    run: &CleanupRunRecord,
    config: &ResolvedConfig,
    now: i64,
) -> Option<CleanupReason> {
    if run.lightweight || matches!(run.status.as_str(), "prepared" | "running") {
        return None;
    }
    if run.status == "completed" && run.sync_status == "synced" && config.cleanup_after_sync {
        return Some(CleanupReason::Done);
    }
    let ttl = i64::from(config.failed_worktree_retention_days) * 86_400;
    let expired = !config.keep_failed_worktrees || now.saturating_sub(run.updated_at_epoch) >= ttl;
    if !expired {
        return None;
    }
    match run.status.as_str() {
        "failed" => Some(CleanupReason::ExpiredFailed),
        "interrupted" => Some(CleanupReason::ExpiredInterrupted),
        "cancelled" => Some(CleanupReason::ExpiredCancelled),
        "stale" => Some(CleanupReason::ExpiredStale),
        _ => None,
    }
}

fn orphan_candidates(
    config: &ResolvedConfig,
    known: &HashSet<PathBuf>,
    active: &HashSet<PathBuf>,
    now: i64,
) -> Result<Vec<(Option<String>, PathBuf, CleanupReason)>, CleanupError> {
    if !config.worktrees_dir.exists() {
        return Ok(Vec::new());
    }
    let ttl = i64::from(config.failed_worktree_retention_days) * 86_400;
    let mut candidates = Vec::new();
    for entry in fs::read_dir(&config.worktrees_dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("run_") || known.contains(&path) || active.contains(&path_key(&path)) {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_secs() as i64)
            .unwrap_or(now);
        if now.saturating_sub(modified) >= ttl {
            candidates.push((Some(name), path, CleanupReason::Orphan));
        }
    }
    Ok(candidates)
}

fn absolute_worktree(config: &ResolvedConfig, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        config.repo_root.join(path)
    }
}

fn safe_candidate(root: &Path, candidate: &Path) -> Result<PathBuf, CleanupError> {
    if fs::symlink_metadata(candidate)?.file_type().is_symlink() {
        return Err(CleanupError::UnsafePath(candidate.to_path_buf()));
    }
    let root = root.canonicalize()?;
    let candidate = candidate.canonicalize()?;
    if candidate == root || !candidate.starts_with(&root) {
        return Err(CleanupError::UnsafePath(candidate));
    }
    Ok(candidate)
}

fn path_key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn registered_worktrees(config: &ResolvedConfig) -> Result<HashSet<PathBuf>, CleanupError> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&config.repo_root)
        .output()?;
    if !output.status.success() {
        return Ok(HashSet::new());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(PathBuf::from)
        .map(|path| path_key(&path))
        .collect())
}

fn directory_size(path: &Path) -> Result<u64, std::io::Error> {
    let mut total = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        total += if metadata.is_dir() {
            directory_size(&entry.path())?
        } else {
            metadata.len()
        };
    }
    Ok(total)
}

fn remove_worktree(
    config: &ResolvedConfig,
    path: &Path,
    registered: bool,
) -> Result<(), std::io::Error> {
    let output = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(path)
        .current_dir(&config.repo_root)
        .output()?;
    if output.status.success() || !path.exists() {
        return Ok(());
    }
    if registered {
        return Err(std::io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    fs::remove_dir_all(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SymphonyConfig;
    use crate::state::{NewRunRecord, RunStateStore};

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_git(root: &Path) {
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.email", "test@example.com"]);
        git(root, &["config", "user.name", "Test"]);
        fs::write(root.join("README.md"), "fixture\n").unwrap();
        git(root, &["add", "README.md"]);
        git(root, &["commit", "-m", "init"]);
    }
    use rusqlite::Connection;
    use std::fs;

    fn add_run(config: &ResolvedConfig, id: &str, status: &str, sync: &str) {
        add_run_at(config, id, status, sync, config.worktrees_dir.join(id));
    }

    fn add_run_at(config: &ResolvedConfig, id: &str, status: &str, sync: &str, path: PathBuf) {
        RunStateStore::new(config.state_db.clone())
            .add_run(NewRunRecord {
                run_id: id.to_owned(),
                story_id: "US-092".to_owned(),
                branch: Some(format!("symphony/{id}")),
                worktree: path,
                lightweight: false,
                status: status.to_owned(),
                result_path: None,
                sync_status: sync.to_owned(),
                next_action: "review".to_owned(),
            })
            .unwrap();
    }

    #[test]
    fn locked_registered_worktree_is_preserved_but_unlocked_worktree_is_removed_with_branch() {
        let temp = tempfile::tempdir().unwrap();
        init_git(temp.path());
        let config = SymphonyConfig::default().resolve(temp.path());
        fs::create_dir_all(&config.worktrees_dir).unwrap();
        let locked = config.worktrees_dir.join("run_locked");
        git(
            temp.path(),
            &[
                "worktree",
                "add",
                "-b",
                "symphony/run_locked",
                locked.to_str().unwrap(),
            ],
        );
        git(temp.path(), &["worktree", "lock", locked.to_str().unwrap()]);
        add_run(&config, "run_locked", "completed", "synced");

        let blocked = cleanup_runtime(&config, false).unwrap();
        assert_eq!(blocked.failures(), 1);
        assert!(locked.exists());

        git(
            temp.path(),
            &["worktree", "unlock", locked.to_str().unwrap()],
        );
        let removed = cleanup_runtime(&config, false).unwrap();
        assert_eq!(removed.removed_count(), 1);
        assert!(!locked.exists());
        let branches = Command::new("git")
            .args(["branch", "--list", "symphony/run_locked"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(!String::from_utf8_lossy(&branches.stdout).trim().is_empty());
    }

    #[test]
    fn active_alias_vetoes_terminal_candidate_for_same_path() {
        let temp = tempfile::tempdir().unwrap();
        let config = SymphonyConfig::default().resolve(temp.path());
        let shared = config.worktrees_dir.join("run_shared");
        fs::create_dir_all(&shared).unwrap();
        add_run_at(&config, "run_done", "completed", "synced", shared.clone());
        add_run_at(
            &config,
            "run_active",
            "prepared",
            "not_applied",
            shared.clone(),
        );

        let result = cleanup_runtime(&config, false).unwrap();

        assert_eq!(result.items.len(), 0, "{:#?}", result.items);
        assert!(shared.exists());
    }

    #[test]
    fn zero_day_orphan_is_removed() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = SymphonyConfig::default().resolve(temp.path());
        config.failed_worktree_retention_days = 0;
        let orphan = config.worktrees_dir.join("run_orphan");
        fs::create_dir_all(&orphan).unwrap();

        let result = cleanup_runtime(&config, false).unwrap();

        assert_eq!(result.removed_count(), 1);
        assert!(!orphan.exists());
    }

    #[test]
    fn stale_run_uses_failed_worktree_retention() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = SymphonyConfig::default().resolve(temp.path());
        config.keep_failed_worktrees = false;
        let worktree = config.worktrees_dir.join("run_stale");
        fs::create_dir_all(&worktree).unwrap();
        add_run(&config, "run_stale", "stale", "not_applied");

        let result = cleanup_runtime(&config, true).unwrap();

        assert_eq!(result.items[0].reason, CleanupReason::ExpiredStale);
    }

    #[test]
    fn cleanup_reconciles_expired_external_run_before_selecting_candidates() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = SymphonyConfig::default().resolve(temp.path());
        config.keep_failed_worktrees = false;
        config.external_heartbeat_ttl_seconds = 1;
        let worktree = config.worktrees_dir.join("run_expired_external");
        fs::create_dir_all(&worktree).unwrap();
        add_run(&config, "run_expired_external", "prepared", "not_applied");
        RunStateStore::new(config.state_db.clone())
            .start_external("run_expired_external", "claude-subagent", 1)
            .unwrap();

        let result = cleanup_runtime_at(&config, true, 2).unwrap();

        assert_eq!(result.items[0].reason, CleanupReason::ExpiredStale);
    }

    #[test]
    fn outside_root_candidate_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = SymphonyConfig::default().resolve(temp.path());
        config.keep_failed_worktrees = false;
        fs::create_dir_all(&config.worktrees_dir).unwrap();
        let external = temp.path().join("external-worktree");
        fs::create_dir_all(&external).unwrap();
        add_run_at(
            &config,
            "run_external",
            "failed",
            "not_applied",
            external.clone(),
        );

        let result = cleanup_runtime(&config, false).unwrap();

        assert_eq!(result.failures(), 1);
        assert!(external.exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_candidate_does_not_delete_external_directory() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let mut config = SymphonyConfig::default().resolve(temp.path());
        config.keep_failed_worktrees = false;
        fs::create_dir_all(&config.worktrees_dir).unwrap();
        let external = temp.path().join("external-target");
        fs::create_dir_all(&external).unwrap();
        let linked = config.worktrees_dir.join("run_linked");
        symlink(&external, &linked).unwrap();
        add_run_at(
            &config,
            "run_linked",
            "failed",
            "not_applied",
            linked.clone(),
        );

        let result = cleanup_runtime(&config, false).unwrap();

        assert_eq!(result.failures(), 1);
        assert!(linked.exists());
        assert!(external.exists());
    }

    #[test]
    fn done_runs_are_removed_and_dry_run_is_non_mutating() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = SymphonyConfig::default().resolve(temp.path());
        config.cleanup_after_sync = true;
        let path = config.worktrees_dir.join("run_done");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("large.bin"), vec![0_u8; 32]).unwrap();
        add_run(&config, "run_done", "completed", "synced");

        let dry = cleanup_runtime(&config, true).unwrap();
        assert_eq!(dry.items.len(), 1);
        assert!(!dry.items[0].removed);
        assert!(path.exists());

        let applied = cleanup_runtime(&config, false).unwrap();
        assert_eq!(applied.removed_count(), 1);
        assert!(applied.reclaimed_bytes() >= 32);
        assert!(!path.exists());
        assert_eq!(cleanup_runtime(&config, false).unwrap().failures(), 0);
    }

    #[test]
    fn recent_failed_is_preserved_but_expired_failed_is_removed() {
        let temp = tempfile::tempdir().unwrap();
        let config = SymphonyConfig::default().resolve(temp.path());
        for id in ["run_recent", "run_old"] {
            fs::create_dir_all(config.worktrees_dir.join(id)).unwrap();
            add_run(&config, id, "failed", "not_applied");
        }
        Connection::open(&config.state_db)
            .unwrap()
            .execute(
                "UPDATE run_state SET updated_at=datetime('now', '-8 days') WHERE run_id='run_old'",
                [],
            )
            .unwrap();

        cleanup_runtime(&config, false).unwrap();

        assert!(config.worktrees_dir.join("run_recent").exists());
        assert!(!config.worktrees_dir.join("run_old").exists());
    }
}
