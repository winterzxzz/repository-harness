use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use thiserror::Error;

use crate::config::{
    ResolvedConfig, AUTO_RETRY_INITIAL_SECONDS, AUTO_RETRY_MAX_SECONDS, AUTO_RETRY_MULTIPLIER,
};
use crate::run::{execute_run, RunError};
use crate::state::{RunStateStore, StateError};
use crate::sync::{refresh_checkout_from_upstream, SyncError};
use crate::work::{HarnessDbWorkSource, WorkError, WorkSource, EXTERNAL_WORK_SOURCE_BOUNDARIES};

#[derive(Debug, Error)]
pub enum AutoError {
    #[error("auto-mode is opt-in. Re-run with --enable to start unattended work polling.")]
    NotEnabled,
    #[error(
        "work source '{0}' is an adapter boundary for future integration; US-045 implements HarnessDbWorkSource first without changing run contracts"
    )]
    AdapterBoundary(String),
    #[error("unsupported work source '{0}'. Supported source: harness-db")]
    UnsupportedSource(String),
    #[error("{0}")]
    State(#[from] StateError),
    #[error("{0}")]
    Work(#[from] WorkError),
    #[error("upstream refresh failed; auto dispatch paused: {0}")]
    UpstreamRefresh(SyncError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoRunOptions {
    pub enabled: bool,
    pub source: String,
    pub once: bool,
    pub max_runs: Option<u32>,
    pub max_attempts: u32,
    pub poll_interval_seconds: u64,
    pub max_idle_cycles: Option<u32>,
    pub retry_initial_seconds: u64,
    pub retry_multiplier: u32,
    pub retry_max_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoRunSummary {
    pub source: String,
    pub enqueued: u32,
    pub completed: u32,
    pub failed: u32,
    pub idle_cycles: u32,
    pub stopped_reason: String,
    pub base_sha: Option<String>,
    pub refresh_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoryRunResult {
    run_id: String,
    outcome: String,
}

trait StoryRunner {
    fn run_story(&mut self, story_id: &str) -> Result<StoryRunResult, String>;
}

struct SymphonyStoryRunner<'a> {
    config: &'a ResolvedConfig,
}

impl StoryRunner for SymphonyStoryRunner<'_> {
    fn run_story(&mut self, story_id: &str) -> Result<StoryRunResult, String> {
        execute_run(self.config, story_id)
            .map(|completed| StoryRunResult {
                run_id: completed.prepared.run_id,
                outcome: completed.outcome,
            })
            .map_err(format_run_error)
    }
}

pub fn options_from_config(config: &ResolvedConfig) -> AutoRunOptions {
    AutoRunOptions {
        enabled: false,
        source: config.auto_source.clone(),
        once: false,
        max_runs: None,
        max_attempts: config.auto_max_attempts,
        poll_interval_seconds: config.auto_poll_interval_seconds,
        max_idle_cycles: None,
        retry_initial_seconds: AUTO_RETRY_INITIAL_SECONDS,
        retry_multiplier: AUTO_RETRY_MULTIPLIER,
        retry_max_seconds: AUTO_RETRY_MAX_SECONDS,
    }
}

pub fn run_auto_mode(
    config: &ResolvedConfig,
    options: AutoRunOptions,
) -> Result<AutoRunSummary, AutoError> {
    let mut runner = SymphonyStoryRunner { config };
    run_auto_mode_with_runner(config, options, &mut runner)
}

fn run_auto_mode_with_runner(
    config: &ResolvedConfig,
    options: AutoRunOptions,
    runner: &mut dyn StoryRunner,
) -> Result<AutoRunSummary, AutoError> {
    let mut refresh = refresh_checkout_from_upstream;
    run_auto_mode_with_runner_at_and_refresh(config, options, runner, unix_timestamp, &mut refresh)
}

#[cfg(test)]
fn run_auto_mode_with_runner_and_refresh(
    config: &ResolvedConfig,
    options: AutoRunOptions,
    runner: &mut dyn StoryRunner,
    mut refresh: impl FnMut(&ResolvedConfig) -> Result<bool, SyncError>,
) -> Result<AutoRunSummary, AutoError> {
    run_auto_mode_with_runner_at_and_refresh(config, options, runner, unix_timestamp, &mut refresh)
}

#[cfg(test)]
fn run_auto_mode_with_runner_at(
    config: &ResolvedConfig,
    options: AutoRunOptions,
    runner: &mut dyn StoryRunner,
    now: impl FnMut() -> i64,
) -> Result<AutoRunSummary, AutoError> {
    let mut refresh = refresh_checkout_from_upstream;
    run_auto_mode_with_runner_at_and_refresh(config, options, runner, now, &mut refresh)
}

fn run_auto_mode_with_runner_at_and_refresh(
    config: &ResolvedConfig,
    options: AutoRunOptions,
    runner: &mut dyn StoryRunner,
    now: impl FnMut() -> i64,
    refresh: &mut dyn FnMut(&ResolvedConfig) -> Result<bool, SyncError>,
) -> Result<AutoRunSummary, AutoError> {
    let heartbeat_interval = Duration::from_secs(
        u64::from(config.agent_timeout_minutes)
            .saturating_mul(20)
            .max(1),
    );
    run_auto_mode_with_runner_at_and_heartbeat_interval(
        config,
        options,
        runner,
        now,
        heartbeat_interval,
        refresh,
    )
}

fn run_auto_mode_with_runner_at_and_heartbeat_interval(
    config: &ResolvedConfig,
    options: AutoRunOptions,
    runner: &mut dyn StoryRunner,
    now: impl FnMut() -> i64,
    heartbeat_interval: Duration,
    refresh: &mut dyn FnMut(&ResolvedConfig) -> Result<bool, SyncError>,
) -> Result<AutoRunSummary, AutoError> {
    run_auto_mode_with_runner_at_and_heartbeat(
        config,
        options,
        runner,
        now,
        heartbeat_interval,
        unix_timestamp,
        None,
        refresh,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_auto_mode_with_runner_at_and_heartbeat(
    config: &ResolvedConfig,
    options: AutoRunOptions,
    runner: &mut dyn StoryRunner,
    mut now: impl FnMut() -> i64,
    heartbeat_interval: Duration,
    heartbeat_now: fn() -> i64,
    heartbeat_ready: Option<mpsc::Sender<()>>,
    refresh: &mut dyn FnMut(&ResolvedConfig) -> Result<bool, SyncError>,
) -> Result<AutoRunSummary, AutoError> {
    if !options.enabled {
        return Err(AutoError::NotEnabled);
    }
    validate_source(&options.source)?;

    let store = RunStateStore::new(config.state_db.clone());
    let owner_pid = std::process::id();
    let owner_token = worker_owner_token(owner_pid);
    let lease_seconds = u64::from(config.agent_timeout_minutes)
        .saturating_mul(60)
        .saturating_add(60);
    store.recover_expired_work_at(
        now(),
        &owner_token,
        owner_pid,
        options.retry_initial_seconds,
        options.retry_multiplier,
        options.retry_max_seconds,
    )?;
    let source = HarnessDbWorkSource::new(&config.harness_db);
    let mut summary = AutoRunSummary {
        source: source.name().to_owned(),
        enqueued: 0,
        completed: 0,
        failed: 0,
        idle_cycles: 0,
        stopped_reason: String::new(),
        base_sha: current_base_sha(config),
        refresh_warning: None,
    };

    loop {
        // Unattended polling is the one place a stale base compounds: without
        // a refresh, days of auto runs would branch from an old HEAD. Prepare
        // itself intentionally does not pull, so pull once per poll cycle
        // here; a failed pull (offline, dirty checkout) degrades to running
        // from the current HEAD instead of stopping the daemon.
        if let Err(error) = refresh(config) {
            if !config.auto_allow_stale_base {
                return Err(AutoError::UpstreamRefresh(error));
            }
            let warning = format!(
                "could not refresh checkout from upstream: {error}; continuing from base {}",
                summary.base_sha.as_deref().unwrap_or("unknown")
            );
            eprintln!("warning: {warning}");
            summary.refresh_warning = Some(warning);
        }
        summary.base_sha = current_base_sha(config);
        for candidate in source.poll()? {
            let queued =
                store.enqueue_work(&candidate.story_id, &candidate.source, options.max_attempts)?;
            if queued.status == "queued" && queued.attempts == 0 {
                summary.enqueued += 1;
            }
        }

        let Some(item) =
            store.claim_next_queued_work_at(now(), &owner_token, owner_pid, lease_seconds)?
        else {
            summary.idle_cycles += 1;
            if options.once {
                summary.stopped_reason = "one poll completed with no queued work".to_owned();
                break;
            }
            if options
                .max_idle_cycles
                .is_some_and(|max_idle| summary.idle_cycles >= max_idle)
            {
                summary.stopped_reason = "max idle cycles reached".to_owned();
                break;
            }
            thread::sleep(Duration::from_secs(options.poll_interval_seconds));
            continue;
        };

        summary.idle_cycles = 0;
        let heartbeat_store = RunStateStore::new(config.state_db.clone());
        let heartbeat_story_id = item.story_id.clone();
        let heartbeat_owner_token = owner_token.clone();
        let heartbeat_ready = heartbeat_ready.clone();
        let (stop_tx, stop_rx) = mpsc::channel();
        let heartbeat = thread::spawn(move || loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }
            let _ = heartbeat_store.refresh_queue_lease_at(
                &heartbeat_story_id,
                &heartbeat_owner_token,
                heartbeat_now(),
                lease_seconds,
            );
            if let Some(ready) = heartbeat_ready.as_ref() {
                let _ = ready.send(());
            }
            if stop_rx.recv_timeout(heartbeat_interval).is_ok() {
                break;
            }
        });
        let run_result = runner.run_story(&item.story_id);
        let _ = stop_tx.send(());
        let _ = heartbeat.join();

        match run_result {
            Ok(result) if result.outcome == "completed" => {
                store.mark_queue_completed(&item.story_id, &result.run_id, &owner_token)?;
                summary.completed += 1;
            }
            Ok(result) => {
                let queue = store.mark_queue_failed_at(
                    &item.story_id,
                    Some(&result.run_id),
                    &format!("run outcome was {}", result.outcome),
                    &owner_token,
                    now(),
                    options.retry_initial_seconds,
                    options.retry_multiplier,
                    options.retry_max_seconds,
                )?;
                if queue.status == "failed" {
                    summary.failed += 1;
                }
            }
            Err(error) => {
                let queue = store.mark_queue_failed_at(
                    &item.story_id,
                    None,
                    &error,
                    &owner_token,
                    now(),
                    options.retry_initial_seconds,
                    options.retry_multiplier,
                    options.retry_max_seconds,
                )?;
                if queue.status == "failed" {
                    summary.failed += 1;
                }
            }
        }

        if options.once {
            summary.stopped_reason = "one queued run processed".to_owned();
            break;
        }
        if options
            .max_runs
            .is_some_and(|max_runs| summary.completed + summary.failed >= max_runs)
        {
            summary.stopped_reason = "max runs reached".to_owned();
            break;
        }
    }

    Ok(summary)
}

fn validate_source(source: &str) -> Result<(), AutoError> {
    if source == "harness-db" {
        return Ok(());
    }
    if EXTERNAL_WORK_SOURCE_BOUNDARIES.contains(&source) {
        return Err(AutoError::AdapterBoundary(source.to_owned()));
    }
    Err(AutoError::UnsupportedSource(source.to_owned()))
}

fn format_run_error(error: RunError) -> String {
    error.to_string()
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn worker_owner_token(pid: u32) -> String {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{pid}-{nonce}")
}

fn current_base_sha(config: &ResolvedConfig) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&config.repo_root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|sha| !sha.is_empty())
        .or_else(|| Some("unknown".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedConfig;
    use rusqlite::{params, Connection};
    use std::path::Path;

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
            agent_command: vec!["fake-agent".to_owned()],
            agent_timeout_minutes: 120,
            pull_request_create: "ask".to_owned(),
            pull_request_provider: "github".to_owned(),
            pull_request_draft_for: vec![],
            changeset_directory: root.join(".harness/changesets"),
            changeset_render_in_summary: true,
            allow_here_for_tiny: true,
            compact_keep_last: 50,
            keep_failed_worktrees: true,
            cleanup_after_sync: false,
            failed_worktree_retention_days: 7,
            auto_source: "harness-db".to_owned(),
            auto_poll_interval_seconds: 0,
            auto_max_attempts: 2,
            auto_allow_stale_base: true,
        }
    }

    fn write_story_db(path: &Path, id: &str) {
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE story (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL,
                    risk_lane TEXT NOT NULL,
                    verify_command TEXT
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO story (id, title, status, risk_lane, verify_command)
                 VALUES (?1, 'fixture', 'planned', 'normal', 'cargo test');",
                params![id],
            )
            .unwrap();
    }

    #[derive(Default)]
    struct FakeRunner {
        failures_before_success: u32,
        calls: u32,
    }

    impl StoryRunner for FakeRunner {
        fn run_story(&mut self, story_id: &str) -> Result<StoryRunResult, String> {
            self.calls += 1;
            if self.calls <= self.failures_before_success {
                return Err("fixture failure".to_owned());
            }
            Ok(StoryRunResult {
                run_id: format!("run_{story_id}_{}", self.calls),
                outcome: "completed".to_owned(),
            })
        }
    }

    #[test]
    fn auto_mode_requires_explicit_enable_flag() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        let mut runner = FakeRunner::default();
        let options = AutoRunOptions {
            enabled: false,
            source: "harness-db".to_owned(),
            once: true,
            max_runs: Some(1),
            max_attempts: 1,
            poll_interval_seconds: 0,
            max_idle_cycles: Some(1),
            retry_initial_seconds: 10,
            retry_multiplier: 2,
            retry_max_seconds: 300,
        };

        let error = run_auto_mode_with_runner(&config, options, &mut runner).unwrap_err();

        assert!(matches!(error, AutoError::NotEnabled));
    }

    #[test]
    fn external_sources_are_adapter_boundaries() {
        let error = validate_source("github-issues").unwrap_err();

        assert!(matches!(error, AutoError::AdapterBoundary(source) if source == "github-issues"));
    }

    #[test]
    fn harness_db_source_feeds_one_queued_run() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        write_story_db(&config.harness_db, "US-AUTO");
        let mut runner = FakeRunner::default();
        let options = AutoRunOptions {
            enabled: true,
            source: "harness-db".to_owned(),
            once: true,
            max_runs: Some(1),
            max_attempts: 2,
            poll_interval_seconds: 0,
            max_idle_cycles: Some(1),
            retry_initial_seconds: 10,
            retry_multiplier: 2,
            retry_max_seconds: 300,
        };

        let summary = run_auto_mode_with_runner(&config, options, &mut runner).unwrap();

        assert_eq!(summary.enqueued, 1);
        assert_eq!(summary.completed, 1);
        assert_eq!(summary.failed, 0);
        let queue = RunStateStore::new(config.state_db)
            .queue_record("US-AUTO")
            .unwrap();
        assert_eq!(queue.status, "completed");
        assert_eq!(queue.attempts, 1);
    }

    #[test]
    fn failed_run_is_retried_until_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        write_story_db(&config.harness_db, "US-RETRY");
        let mut runner = FakeRunner {
            failures_before_success: 1,
            calls: 0,
        };
        let options = AutoRunOptions {
            enabled: true,
            source: "harness-db".to_owned(),
            once: false,
            max_runs: Some(1),
            max_attempts: 2,
            poll_interval_seconds: 0,
            max_idle_cycles: Some(1),
            retry_initial_seconds: 0,
            retry_multiplier: 2,
            retry_max_seconds: 0,
        };

        let summary = run_auto_mode_with_runner(&config, options, &mut runner).unwrap();

        assert_eq!(summary.completed, 1);
        assert_eq!(runner.calls, 2);
        let queue = RunStateStore::new(config.state_db)
            .queue_record("US-RETRY")
            .unwrap();
        assert_eq!(queue.status, "completed");
        assert_eq!(queue.attempts, 2);
    }

    #[test]
    fn auto_startup_recovers_expired_queue_and_active_run_at_injected_time() {
        use crate::state::NewRunRecord;

        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        write_story_db(&config.harness_db, "US-DONE");
        Connection::open(&config.harness_db)
            .unwrap()
            .execute("UPDATE story SET status='implemented'", [])
            .unwrap();
        let store = RunStateStore::new(config.state_db.clone());
        store.enqueue_work("US-ORPHAN", "harness-db", 2).unwrap();
        store
            .claim_next_queued_work_at(100, "dead-owner", std::process::id(), 10)
            .unwrap();
        store
            .add_run(NewRunRecord {
                run_id: "run_orphan".to_owned(),
                story_id: "US-ORPHAN".to_owned(),
                branch: None,
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "running".to_owned(),
                result_path: None,
                sync_status: "not_applicable".to_owned(),
                next_action: "continue run".to_owned(),
            })
            .unwrap();
        let mut runner = FakeRunner::default();
        let options = AutoRunOptions {
            enabled: true,
            source: "harness-db".to_owned(),
            once: true,
            max_runs: None,
            max_attempts: 2,
            poll_interval_seconds: 0,
            max_idle_cycles: Some(1),
            retry_initial_seconds: 10,
            retry_multiplier: 2,
            retry_max_seconds: 300,
        };

        run_auto_mode_with_runner_at(&config, options, &mut runner, || 111).unwrap();

        assert_eq!(store.show_run("run_orphan").unwrap().status, "interrupted");
        let queue = store.queue_record("US-ORPHAN").unwrap();
        assert_eq!(queue.status, "queued");
        assert_eq!(queue.next_attempt_at, 121);
        assert_eq!(runner.calls, 0);
    }

    #[test]
    fn blocking_runner_receives_lease_heartbeats() {
        struct HeartbeatObservingRunner {
            state_db: std::path::PathBuf,
            heartbeat_ready: mpsc::Receiver<()>,
            observed: bool,
        }

        impl StoryRunner for HeartbeatObservingRunner {
            fn run_story(&mut self, story_id: &str) -> Result<StoryRunResult, String> {
                self.heartbeat_ready
                    .recv()
                    .map_err(|error| error.to_string())?;
                self.observed = RunStateStore::new(self.state_db.clone())
                    .queue_record(story_id)
                    .map_err(|error| error.to_string())?
                    .heartbeat_at
                    == Some(105);
                Ok(StoryRunResult {
                    run_id: "run_heartbeat".to_owned(),
                    outcome: "completed".to_owned(),
                })
            }
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = config_for_root(temp_dir.path());
        config.agent_timeout_minutes = 0;
        write_story_db(&config.harness_db, "US-HEARTBEAT");
        let (heartbeat_ready_tx, heartbeat_ready_rx) = mpsc::channel();
        let mut runner = HeartbeatObservingRunner {
            state_db: config.state_db.clone(),
            heartbeat_ready: heartbeat_ready_rx,
            observed: false,
        };
        let options = AutoRunOptions {
            enabled: true,
            source: "harness-db".to_owned(),
            once: true,
            max_runs: Some(1),
            max_attempts: 1,
            poll_interval_seconds: 0,
            max_idle_cycles: Some(1),
            retry_initial_seconds: 10,
            retry_multiplier: 2,
            retry_max_seconds: 300,
        };

        run_auto_mode_with_runner_at_and_heartbeat(
            &config,
            options,
            &mut runner,
            || 100,
            Duration::ZERO,
            || 105,
            Some(heartbeat_ready_tx),
            &mut |_| Ok(false),
        )
        .unwrap();

        assert!(runner.observed);
    }

    #[test]
    fn failing_runner_stops_heartbeat_before_requeueing() {
        struct FailingRunner;

        impl StoryRunner for FailingRunner {
            fn run_story(&mut self, _story_id: &str) -> Result<StoryRunResult, String> {
                Err("runner failed".to_owned())
            }
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        write_story_db(&config.harness_db, "US-HEARTBEAT-ERROR");
        let options = AutoRunOptions {
            enabled: true,
            source: "harness-db".to_owned(),
            once: true,
            max_runs: Some(1),
            max_attempts: 2,
            poll_interval_seconds: 0,
            max_idle_cycles: Some(1),
            retry_initial_seconds: 10,
            retry_multiplier: 2,
            retry_max_seconds: 300,
        };

        run_auto_mode_with_runner_at_and_heartbeat_interval(
            &config,
            options,
            &mut FailingRunner,
            || 100,
            Duration::ZERO,
            &mut |_| Ok(false),
        )
        .unwrap();

        let queue = RunStateStore::new(config.state_db)
            .queue_record("US-HEARTBEAT-ERROR")
            .unwrap();
        assert_eq!(queue.status, "queued");
        assert_eq!(queue.owner_token, None);
        assert_eq!(queue.heartbeat_at, None);
        assert_eq!(queue.lease_expires_at, None);
    }

    #[test]
    fn refresh_failure_stops_before_polling_or_dispatch_by_default() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        let mut config = config;
        config.auto_allow_stale_base = false;
        write_story_db(&config.harness_db, "US-STALE");
        let mut runner = FakeRunner::default();
        let options = AutoRunOptions {
            enabled: true,
            source: "harness-db".to_owned(),
            once: true,
            max_runs: Some(1),
            max_attempts: 1,
            poll_interval_seconds: 0,
            max_idle_cycles: Some(1),
            retry_initial_seconds: 10,
            retry_multiplier: 2,
            retry_max_seconds: 300,
        };

        let error = run_auto_mode_with_runner_and_refresh(&config, options, &mut runner, |_| {
            Err(crate::sync::SyncError::GitFailed("offline".to_owned()))
        })
        .unwrap_err();

        assert!(matches!(error, AutoError::UpstreamRefresh(_)));
        assert_eq!(runner.calls, 0);
        assert!(matches!(
            RunStateStore::new(config.state_db).queue_record("US-STALE"),
            Err(StateError::RunNotFound(id)) if id == "US-STALE"
        ));
    }

    #[test]
    fn stale_base_opt_in_continues_and_reports_refresh_warning() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = config_for_root(temp_dir.path());
        config.auto_allow_stale_base = true;
        write_story_db(&config.harness_db, "US-STALE-OK");
        let mut runner = FakeRunner::default();
        let options = AutoRunOptions {
            enabled: true,
            source: "harness-db".to_owned(),
            once: true,
            max_runs: Some(1),
            max_attempts: 1,
            poll_interval_seconds: 0,
            max_idle_cycles: Some(1),
            retry_initial_seconds: 10,
            retry_multiplier: 2,
            retry_max_seconds: 300,
        };

        let summary = run_auto_mode_with_runner_and_refresh(&config, options, &mut runner, |_| {
            Err(crate::sync::SyncError::GitFailed("offline".to_owned()))
        })
        .unwrap();

        assert_eq!(runner.calls, 1);
        assert_eq!(summary.base_sha.as_deref(), Some("unknown"));
        assert!(summary
            .refresh_warning
            .as_deref()
            .unwrap()
            .contains("offline"));
    }
}
