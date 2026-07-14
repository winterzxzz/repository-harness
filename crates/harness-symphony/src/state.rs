use std::fs;
use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error(
        "active run already exists: {0}. Finish, cancel, or fail that run before starting another."
    )]
    ActiveRunExists(String),
    #[error("run not found: {0}")]
    RunNotFound(String),
    #[error("auto queue ownership lost for story {0}")]
    QueueOwnershipLost(String),
    #[error("run {id} cannot be replaced because status is {status}; only completed runs can request changes")]
    RunNotReplaceable { id: String, status: String },
    #[error(
        "replacement story mismatch for {source_run_id}: source is {source_story_id}, replacement is {replacement_story_id}"
    )]
    ReplacementStoryMismatch {
        source_run_id: String,
        source_story_id: String,
        replacement_story_id: String,
    },
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRecord {
    pub run_id: String,
    pub story_id: String,
    pub branch: Option<String>,
    pub worktree: PathBuf,
    pub lightweight: bool,
    pub status: String,
    pub result_path: Option<PathBuf>,
    pub pr_url: Option<String>,
    pub pr_status: String,
    pub sync_status: String,
    pub next_action: String,
    pub agent: String,
    pub owner_pid: Option<u32>,
    pub agent_pid: Option<u32>,
    pub agent_start_identity: Option<String>,
    pub heartbeat_at: Option<i64>,
    pub current_stage: String,
    pub cancel_requested: bool,
    pub terminal_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRecord {
    pub story_id: String,
    pub source: String,
    pub status: String,
    pub attempts: u32,
    pub max_attempts: u32,
    pub last_run_id: Option<String>,
    pub last_error: Option<String>,
    pub next_attempt_at: i64,
    pub owner_token: Option<String>,
    pub owner_pid: Option<u32>,
    pub owner_start_identity: Option<String>,
    pub heartbeat_at: Option<i64>,
    pub lease_expires_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct NewRunRecord {
    pub run_id: String,
    pub story_id: String,
    pub branch: Option<String>,
    pub worktree: PathBuf,
    pub lightweight: bool,
    pub status: String,
    pub result_path: Option<PathBuf>,
    pub sync_status: String,
    pub next_action: String,
}

pub struct RunStateStore {
    path: PathBuf,
}

impl RunStateStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn init(&self) -> Result<(), StateError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(&self.path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("BEGIN IMMEDIATE;")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS run_state (
                run_id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                branch TEXT,
                worktree TEXT NOT NULL,
                lightweight INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                result_path TEXT,
                pr_url TEXT,
                pr_status TEXT NOT NULL DEFAULT 'missing',
                sync_status TEXT NOT NULL DEFAULT 'not_applicable',
                next_action TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS changeset_sync (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                applied INTEGER NOT NULL,
                synced_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS auto_queue (
                story_id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                status TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                max_attempts INTEGER NOT NULL,
                last_run_id TEXT,
                last_error TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )?;
        ensure_column(
            &connection,
            "run_state",
            "lightweight",
            "ALTER TABLE run_state ADD COLUMN lightweight INTEGER NOT NULL DEFAULT 0;",
        )?;
        ensure_column(
            &connection,
            "auto_queue",
            "next_attempt_at",
            "ALTER TABLE auto_queue ADD COLUMN next_attempt_at INTEGER NOT NULL DEFAULT 0;",
        )?;
        ensure_column(
            &connection,
            "auto_queue",
            "owner_token",
            "ALTER TABLE auto_queue ADD COLUMN owner_token TEXT;",
        )?;
        ensure_column(
            &connection,
            "auto_queue",
            "owner_pid",
            "ALTER TABLE auto_queue ADD COLUMN owner_pid INTEGER;",
        )?;
        ensure_column(
            &connection,
            "auto_queue",
            "owner_start_identity",
            "ALTER TABLE auto_queue ADD COLUMN owner_start_identity TEXT;",
        )?;
        ensure_column(
            &connection,
            "auto_queue",
            "heartbeat_at",
            "ALTER TABLE auto_queue ADD COLUMN heartbeat_at INTEGER;",
        )?;
        ensure_column(
            &connection,
            "auto_queue",
            "lease_expires_at",
            "ALTER TABLE auto_queue ADD COLUMN lease_expires_at INTEGER;",
        )?;
        ensure_column(
            &connection,
            "run_state",
            "pr_status",
            "ALTER TABLE run_state ADD COLUMN pr_status TEXT NOT NULL DEFAULT 'missing';",
        )?;
        ensure_column(
            &connection,
            "run_state",
            "agent",
            "ALTER TABLE run_state ADD COLUMN agent TEXT NOT NULL DEFAULT 'codex';",
        )?;
        for (name, sql) in [
            (
                "owner_pid",
                "ALTER TABLE run_state ADD COLUMN owner_pid INTEGER;",
            ),
            (
                "agent_pid",
                "ALTER TABLE run_state ADD COLUMN agent_pid INTEGER;",
            ),
            (
                "agent_start_identity",
                "ALTER TABLE run_state ADD COLUMN agent_start_identity TEXT;",
            ),
            (
                "heartbeat_at",
                "ALTER TABLE run_state ADD COLUMN heartbeat_at INTEGER;",
            ),
            (
                "current_stage",
                "ALTER TABLE run_state ADD COLUMN current_stage TEXT NOT NULL DEFAULT 'start';",
            ),
            (
                "cancel_requested",
                "ALTER TABLE run_state ADD COLUMN cancel_requested INTEGER NOT NULL DEFAULT 0;",
            ),
            (
                "terminal_reason",
                "ALTER TABLE run_state ADD COLUMN terminal_reason TEXT;",
            ),
        ] {
            ensure_column(&connection, "run_state", name, sql)?;
        }
        connection.execute_batch("COMMIT;")?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn add_run(&self, input: NewRunRecord) -> Result<(), StateError> {
        self.init()?;
        let mut connection = Connection::open(&self.path)?;
        // IMMEDIATE takes the write lock up front so two concurrent prepares
        // (e.g. web UI and terminal) cannot both pass the active-run check.
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(active) = active_run_id(&transaction)? {
            return Err(StateError::ActiveRunExists(active));
        }
        insert_run(&transaction, input, None)?;
        transaction.commit()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn replace_run(
        &self,
        source_run_id: &str,
        rejection_reason: &str,
        replacement: NewRunRecord,
    ) -> Result<(), StateError> {
        self.replace_run_with_agent(source_run_id, rejection_reason, replacement, "codex")
    }

    pub fn replace_run_with_agent(
        &self,
        source_run_id: &str,
        rejection_reason: &str,
        replacement: NewRunRecord,
        agent: &str,
    ) -> Result<(), StateError> {
        self.init()?;
        let mut connection = Connection::open(&self.path)?;
        let transaction = connection.transaction()?;
        if let Some(active) = active_run_id(&transaction)? {
            return Err(StateError::ActiveRunExists(active));
        }
        let source = transaction
            .query_row(
                "SELECT story_id, status FROM run_state WHERE run_id=?1;",
                params![source_run_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or_else(|| StateError::RunNotFound(source_run_id.to_owned()))?;
        if source.1 != "completed" {
            return Err(StateError::RunNotReplaceable {
                id: source_run_id.to_owned(),
                status: source.1,
            });
        }
        if source.0 != replacement.story_id {
            return Err(StateError::ReplacementStoryMismatch {
                source_run_id: source_run_id.to_owned(),
                source_story_id: source.0,
                replacement_story_id: replacement.story_id,
            });
        }

        insert_run(&transaction, replacement, Some(agent))?;
        transaction.execute(
            "UPDATE run_state
             SET status='rejected', next_action=?1, updated_at=datetime('now')
             WHERE run_id=?2;",
            params![
                format!("changes requested: {rejection_reason}"),
                source_run_id
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn remove_run(&self, run_id: &str) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute("DELETE FROM run_state WHERE run_id=?1;", params![run_id])?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn update_status(
        &self,
        run_id: &str,
        status: &str,
        next_action: &str,
    ) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE run_state
             SET status=?1, next_action=?2, updated_at=datetime('now')
             WHERE run_id=?3;",
            params![status, next_action, run_id],
        )?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    pub fn begin_execution(
        &self,
        run_id: &str,
        owner_pid: u32,
        agent_pid: u32,
        agent_start_identity: &str,
        now: i64,
    ) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE run_state SET status='running', owner_pid=?2, agent_pid=?3,
             agent_start_identity=?4, heartbeat_at=?5, current_stage='agent',
             cancel_requested=0, terminal_reason=NULL, updated_at=datetime('now')
             WHERE run_id=?1 AND status IN ('prepared','running')",
            params![
                run_id,
                i64::from(owner_pid),
                i64::from(agent_pid),
                agent_start_identity,
                now
            ],
        )?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    pub fn refresh_heartbeat(&self, run_id: &str, now: i64) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute("UPDATE run_state SET heartbeat_at=?2, updated_at=datetime('now') WHERE run_id=?1 AND status='running'", params![run_id, now])?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    pub fn set_stage(&self, run_id: &str, stage: &str) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE run_state SET current_stage=?2, updated_at=datetime('now') WHERE run_id=?1",
            params![run_id, stage],
        )?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    pub fn request_cancel(&self, run_id: &str) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute("UPDATE run_state SET cancel_requested=1, updated_at=datetime('now') WHERE run_id=?1 AND status IN ('prepared','running')", params![run_id])?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    pub fn cancellation_requested(&self, run_id: &str) -> Result<bool, StateError> {
        Ok(self.show_run(run_id)?.cancel_requested)
    }

    pub fn finish_execution(
        &self,
        run_id: &str,
        status: &str,
        reason: &str,
    ) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE run_state SET status=?2, next_action=?3, terminal_reason=?3,
             owner_pid=NULL, agent_pid=NULL, agent_start_identity=NULL, heartbeat_at=NULL,
             cancel_requested=0, updated_at=datetime('now') WHERE run_id=?1",
            params![run_id, status, reason],
        )?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    pub fn list_runs(&self) -> Result<Vec<RunRecord>, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        let mut statement = connection.prepare(
            "SELECT run_id, story_id, branch, worktree, lightweight, status, result_path,
                    pr_url, pr_status, sync_status, next_action, agent, owner_pid, agent_pid,
                    agent_start_identity, heartbeat_at, current_stage, cancel_requested, terminal_reason
             FROM run_state
             ORDER BY created_at DESC, run_id DESC;",
        )?;
        let rows = statement.query_map([], run_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StateError::from)
    }

    pub fn show_run(&self, run_id: &str) -> Result<RunRecord, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection
            .query_row(
                "SELECT run_id, story_id, branch, worktree, lightweight, status, result_path,
                        pr_url, pr_status, sync_status, next_action, agent, owner_pid, agent_pid,
                        agent_start_identity, heartbeat_at, current_stage, cancel_requested, terminal_reason
                 FROM run_state
                 WHERE run_id=?1;",
                params![run_id],
                run_from_row,
            )
            .optional()?
            .ok_or_else(|| StateError::RunNotFound(run_id.to_owned()))
    }

    pub fn active_run(&self) -> Result<Option<RunRecord>, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection
            .query_row(
                "SELECT run_id, story_id, branch, worktree, lightweight, status, result_path,
                        pr_url, pr_status, sync_status, next_action, agent, owner_pid, agent_pid,
                        agent_start_identity, heartbeat_at, current_stage, cancel_requested, terminal_reason
                 FROM run_state
                 WHERE status IN ('prepared', 'running')
                 ORDER BY created_at ASC
                 LIMIT 1;",
                [],
                run_from_row,
            )
            .optional()
            .map_err(StateError::from)
    }

    pub fn record_run_agent(&self, run_id: &str, agent: &str) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE run_state
             SET agent=?1, updated_at=datetime('now')
             WHERE run_id=?2;",
            params![agent, run_id],
        )?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection
            .query_row(
                "SELECT value FROM settings WHERE key=?1;",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(StateError::from)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "INSERT INTO settings (key, value, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET
                value=excluded.value,
                updated_at=datetime('now');",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn update_pr_url(&self, run_id: &str, pr_url: &str) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE run_state
             SET pr_url=?1, pr_status='created', next_action='review pull request', updated_at=datetime('now')
             WHERE run_id=?2;",
            params![pr_url, run_id],
        )?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    pub fn update_pr_status(&self, run_id: &str, pr_status: &str) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        let next_action = if pr_status == "merged" {
            "approve sync"
        } else if pr_status == "failed" {
            "retry pull request creation"
        } else if pr_status == "not_applicable" {
            "review local run artifacts"
        } else {
            "review pull request"
        };
        connection.execute(
            "UPDATE run_state
             SET pr_status=?1, next_action=?2, updated_at=datetime('now')
             WHERE run_id=?3;",
            params![pr_status, next_action, run_id],
        )?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    pub fn record_pr_failure(&self, run_id: &str, error: &str) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE run_state
             SET pr_status='failed', next_action=?1, updated_at=datetime('now')
             WHERE run_id=?2;",
            params![format!("pull request creation failed: {error}"), run_id],
        )?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    pub fn update_sync_status(
        &self,
        run_id: &str,
        sync_status: &str,
        next_action: &str,
    ) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE run_state
             SET sync_status=?1, next_action=?2, updated_at=datetime('now')
             WHERE run_id=?3;",
            params![sync_status, next_action, run_id],
        )?;
        if connection.changes() == 0 {
            return Err(StateError::RunNotFound(run_id.to_owned()));
        }
        Ok(())
    }

    pub fn record_changeset_synced(
        &self,
        id: &str,
        path: &std::path::Path,
        applied: bool,
    ) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "INSERT INTO changeset_sync (id, path, applied, synced_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(id) DO UPDATE SET
                path=excluded.path,
                applied=excluded.applied,
                synced_at=datetime('now');",
            params![id, path.display().to_string(), i64::from(applied)],
        )?;
        Ok(())
    }

    pub fn changeset_synced(&self, id: &str) -> Result<bool, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection
            .query_row(
                "SELECT 1 FROM changeset_sync WHERE id=?1;",
                params![id],
                |_| Ok(()),
            )
            .optional()
            .map(|value| value.is_some())
            .map_err(StateError::from)
    }

    pub fn enqueue_work(
        &self,
        story_id: &str,
        source: &str,
        max_attempts: u32,
    ) -> Result<QueueRecord, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "INSERT INTO auto_queue (story_id, source, status, max_attempts)
             VALUES (?1, ?2, 'queued', ?3)
             ON CONFLICT(story_id) DO UPDATE SET
                source=excluded.source,
                status=CASE
                    WHEN auto_queue.status IN ('completed', 'running') THEN auto_queue.status
                    WHEN auto_queue.attempts >= auto_queue.max_attempts THEN 'failed'
                    ELSE 'queued'
                END,
                max_attempts=excluded.max_attempts,
                updated_at=datetime('now');",
            params![story_id, source, i64::from(max_attempts)],
        )?;
        self.queue_record(story_id)
    }

    #[allow(dead_code)]
    pub fn next_queued_work(&self) -> Result<Option<QueueRecord>, StateError> {
        self.next_queued_work_at(unix_timestamp())
    }

    #[allow(dead_code)]
    pub fn next_queued_work_at(&self, now: i64) -> Result<Option<QueueRecord>, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection
            .query_row(
                "SELECT story_id, source, status, attempts, max_attempts, last_run_id, last_error,
                        next_attempt_at, owner_token, owner_pid, owner_start_identity,
                        heartbeat_at, lease_expires_at
                 FROM auto_queue
                 WHERE status='queued' AND attempts < max_attempts AND next_attempt_at <= ?1
                 ORDER BY created_at ASC, story_id ASC
                 LIMIT 1;",
                params![now],
                queue_from_row,
            )
            .optional()
            .map_err(StateError::from)
    }

    pub fn claim_next_queued_work_at(
        &self,
        now: i64,
        owner_token: &str,
        owner_pid: u32,
        lease_seconds: u64,
    ) -> Result<Option<QueueRecord>, StateError> {
        self.init()?;
        let mut connection = Connection::open(&self.path)?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let owner_start_identity = match probe_process(owner_pid) {
            ProcessProbe::Live(identity) => Some(identity),
            ProcessProbe::Absent | ProcessProbe::Unknown => None,
        };
        let story_id = transaction
            .query_row(
                "SELECT story_id FROM auto_queue
                 WHERE status='queued' AND attempts < max_attempts AND next_attempt_at <= ?1
                 ORDER BY created_at ASC, story_id ASC LIMIT 1",
                params![now],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(story_id) = story_id else {
            transaction.commit()?;
            return Ok(None);
        };
        transaction.execute(
            "UPDATE auto_queue SET status='running', attempts=attempts+1,
                    last_error=NULL, owner_token=?2, owner_pid=?3, owner_start_identity=?4,
                    heartbeat_at=?5, lease_expires_at=?6, updated_at=datetime('now')
             WHERE story_id=?1 AND status='queued'",
            params![
                story_id,
                owner_token,
                i64::from(owner_pid),
                owner_start_identity,
                now,
                now.saturating_add(lease_seconds as i64)
            ],
        )?;
        let record = transaction.query_row(
            "SELECT story_id, source, status, attempts, max_attempts, last_run_id, last_error,
                    next_attempt_at, owner_token, owner_pid, owner_start_identity,
                    heartbeat_at, lease_expires_at
             FROM auto_queue WHERE story_id=?1",
            params![story_id],
            queue_from_row,
        )?;
        transaction.commit()?;
        Ok(Some(record))
    }

    pub fn refresh_queue_lease_at(
        &self,
        story_id: &str,
        owner_token: &str,
        now: i64,
        lease_seconds: u64,
    ) -> Result<bool, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE auto_queue SET heartbeat_at=?3, lease_expires_at=?4, updated_at=datetime('now')
             WHERE story_id=?1 AND status='running' AND owner_token=?2",
            params![
                story_id,
                owner_token,
                now,
                now.saturating_add(lease_seconds as i64)
            ],
        )?;
        Ok(connection.changes() == 1)
    }

    #[allow(dead_code)]
    pub fn mark_queue_running(&self, story_id: &str) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE auto_queue
             SET status='running', attempts=attempts+1, last_error=NULL, updated_at=datetime('now')
             WHERE story_id=?1;",
            params![story_id],
        )?;
        Ok(())
    }

    pub fn mark_queue_completed(
        &self,
        story_id: &str,
        run_id: &str,
        owner_token: &str,
    ) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE auto_queue
             SET status='completed', last_run_id=?2, last_error=NULL,
                 owner_token=NULL, owner_pid=NULL, owner_start_identity=NULL,
                 heartbeat_at=NULL, lease_expires_at=NULL,
                 updated_at=datetime('now')
             WHERE story_id=?1 AND status='running' AND owner_token=?3;",
            params![story_id, run_id, owner_token],
        )?;
        if connection.changes() != 1 {
            return Err(StateError::QueueOwnershipLost(story_id.to_owned()));
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn mark_queue_failed(
        &self,
        story_id: &str,
        run_id: Option<&str>,
        error: &str,
        owner_token: &str,
    ) -> Result<QueueRecord, StateError> {
        self.mark_queue_failed_at(
            story_id,
            run_id,
            error,
            owner_token,
            unix_timestamp(),
            0,
            1,
            0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn mark_queue_failed_at(
        &self,
        story_id: &str,
        run_id: Option<&str>,
        error: &str,
        owner_token: &str,
        now: i64,
        initial_seconds: u64,
        multiplier: u32,
        max_seconds: u64,
    ) -> Result<QueueRecord, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        let attempts = connection.query_row(
            "SELECT attempts FROM auto_queue WHERE story_id=?1",
            params![story_id],
            |row| row.get::<_, i64>(0),
        )? as u32;
        let delay = retry_delay(attempts, initial_seconds, multiplier, max_seconds);
        connection.execute(
            "UPDATE auto_queue
             SET status=CASE WHEN attempts >= max_attempts THEN 'failed' ELSE 'queued' END,
                 last_run_id=?2,
                 last_error=?3,
                 next_attempt_at=CASE WHEN attempts >= max_attempts THEN next_attempt_at ELSE ?4 END,
                 owner_token=NULL, owner_pid=NULL, owner_start_identity=NULL,
                 heartbeat_at=NULL, lease_expires_at=NULL,
                 updated_at=datetime('now')
             WHERE story_id=?1 AND status='running' AND owner_token=?5;",
            params![
                story_id,
                run_id,
                error,
                now.saturating_add(delay as i64),
                owner_token
            ],
        )?;
        if connection.changes() != 1 {
            return Err(StateError::QueueOwnershipLost(story_id.to_owned()));
        }
        self.queue_record(story_id)
    }

    pub fn recover_expired_work_at(
        &self,
        now: i64,
        current_owner_token: &str,
        current_owner_pid: u32,
        initial_seconds: u64,
        multiplier: u32,
        max_seconds: u64,
    ) -> Result<u32, StateError> {
        self.recover_expired_work_at_with_identity(
            now,
            current_owner_token,
            current_owner_pid,
            initial_seconds,
            multiplier,
            max_seconds,
            probe_process,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn recover_expired_work_at_with_identity(
        &self,
        now: i64,
        current_owner_token: &str,
        current_owner_pid: u32,
        initial_seconds: u64,
        multiplier: u32,
        max_seconds: u64,
        mut probe: impl FnMut(u32) -> ProcessProbe,
    ) -> Result<u32, StateError> {
        self.init()?;
        let mut connection = Connection::open(&self.path)?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut statement = transaction.prepare(
            "SELECT story_id, last_run_id, owner_token, owner_pid, owner_start_identity
             FROM auto_queue
             WHERE status='running' AND lease_expires_at IS NOT NULL AND lease_expires_at <= ?1",
        )?;
        let expired = statement
            .query_map(params![now], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?.map(|pid| pid as u32),
                    row.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(statement);
        let mut recovered = 0;
        for (story_id, recorded_run_id, owner_token, owner_pid, recorded_start_identity) in &expired
        {
            let owner_is_valid = if owner_token.as_deref() == Some(current_owner_token) {
                true
            } else if owner_pid == &Some(current_owner_pid) {
                false
            } else if let Some(pid) = owner_pid {
                match probe(*pid) {
                    ProcessProbe::Absent => false,
                    ProcessProbe::Live(actual) => recorded_start_identity
                        .as_deref()
                        .is_none_or(|recorded| recorded == actual),
                    // If identity cannot be verified, fail closed and leave the lease untouched.
                    ProcessProbe::Unknown => true,
                }
            } else {
                false
            };
            if owner_is_valid {
                continue;
            }
            let run_id = match recorded_run_id {
                Some(run_id) => Some(run_id.clone()),
                None => transaction
                    .query_row(
                        "SELECT run_id FROM run_state
                         WHERE story_id=?1 AND status IN ('prepared', 'running')
                         ORDER BY created_at DESC LIMIT 1",
                        params![story_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?,
            };
            if let Some(run_id) = run_id {
                transaction.execute(
                    "UPDATE run_state SET status='interrupted', next_action='retry interrupted auto run',
                            updated_at=datetime('now')
                     WHERE run_id=?1 AND status IN ('prepared', 'running')",
                    params![run_id],
                )?;
            }
            let attempts = transaction.query_row(
                "SELECT attempts FROM auto_queue WHERE story_id=?1",
                params![story_id],
                |row| row.get::<_, i64>(0),
            )? as u32;
            let delay = retry_delay(attempts, initial_seconds, multiplier, max_seconds);
            transaction.execute(
                "UPDATE auto_queue SET
                    status=CASE WHEN attempts >= max_attempts THEN 'failed' ELSE 'queued' END,
                    last_error='auto worker lease expired; previous run interrupted',
                    next_attempt_at=CASE WHEN attempts >= max_attempts THEN next_attempt_at ELSE ?2 END,
                    owner_token=NULL, owner_pid=NULL, owner_start_identity=NULL,
                    heartbeat_at=NULL, lease_expires_at=NULL,
                    updated_at=datetime('now') WHERE story_id=?1",
                params![story_id, now.saturating_add(delay as i64)],
            )?;
            recovered += 1;
        }
        transaction.commit()?;
        Ok(recovered)
    }

    pub fn queue_record(&self, story_id: &str) -> Result<QueueRecord, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection
            .query_row(
                "SELECT story_id, source, status, attempts, max_attempts, last_run_id, last_error,
                        next_attempt_at, owner_token, owner_pid, owner_start_identity,
                        heartbeat_at, lease_expires_at
                 FROM auto_queue
                 WHERE story_id=?1;",
                params![story_id],
                queue_from_row,
            )
            .optional()?
            .ok_or_else(|| StateError::RunNotFound(story_id.to_owned()))
    }
}

fn insert_run(
    connection: &Connection,
    input: NewRunRecord,
    agent: Option<&str>,
) -> Result<(), StateError> {
    connection.execute(
        "INSERT INTO run_state (
            run_id, story_id, branch, worktree, lightweight, status, result_path,
            sync_status, next_action, agent
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, COALESCE(?10, 'codex'));",
        params![
            input.run_id,
            input.story_id,
            input.branch,
            input.worktree.display().to_string(),
            i64::from(input.lightweight),
            input.status,
            input.result_path.map(|path| path.display().to_string()),
            input.sync_status,
            input.next_action,
            agent,
        ],
    )?;
    Ok(())
}

fn active_run_id(connection: &Connection) -> Result<Option<String>, StateError> {
    connection
        .query_row(
            "SELECT run_id FROM run_state
             WHERE status IN ('prepared', 'running')
             ORDER BY created_at ASC
             LIMIT 1;",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(StateError::from)
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    statement: &str,
) -> Result<(), StateError> {
    let mut columns = connection.prepare(&format!("PRAGMA table_info({table});"))?;
    let names = columns
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if !names.iter().any(|name| name == column) {
        connection.execute_batch(statement)?;
    }
    Ok(())
}

fn run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunRecord> {
    Ok(RunRecord {
        run_id: row.get(0)?,
        story_id: row.get(1)?,
        branch: row.get(2)?,
        worktree: PathBuf::from(row.get::<_, String>(3)?),
        lightweight: row.get::<_, i64>(4)? != 0,
        status: row.get(5)?,
        result_path: row.get::<_, Option<String>>(6)?.map(PathBuf::from),
        pr_url: row.get(7)?,
        pr_status: row.get(8)?,
        sync_status: row.get(9)?,
        next_action: row.get(10)?,
        agent: row.get(11)?,
        owner_pid: row.get::<_, Option<i64>>(12)?.map(|value| value as u32),
        agent_pid: row.get::<_, Option<i64>>(13)?.map(|value| value as u32),
        agent_start_identity: row.get(14)?,
        heartbeat_at: row.get(15)?,
        current_stage: row.get(16)?,
        cancel_requested: row.get::<_, i64>(17)? != 0,
        terminal_reason: row.get(18)?,
    })
}

fn queue_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<QueueRecord> {
    Ok(QueueRecord {
        story_id: row.get(0)?,
        source: row.get(1)?,
        status: row.get(2)?,
        attempts: row.get::<_, i64>(3)? as u32,
        max_attempts: row.get::<_, i64>(4)? as u32,
        last_run_id: row.get(5)?,
        last_error: row.get(6)?,
        next_attempt_at: row.get(7)?,
        owner_token: row.get(8)?,
        owner_pid: row.get::<_, Option<i64>>(9)?.map(|value| value as u32),
        owner_start_identity: row.get(10)?,
        heartbeat_at: row.get(11)?,
        lease_expires_at: row.get(12)?,
    })
}

fn retry_delay(attempts: u32, initial: u64, multiplier: u32, maximum: u64) -> u64 {
    let exponent = attempts.saturating_sub(1);
    let factor = u64::from(multiplier).saturating_pow(exponent);
    initial.saturating_mul(factor).min(maximum)
}

#[allow(dead_code)]
fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProcessProbe {
    Live(String),
    Absent,
    Unknown,
}

#[cfg(unix)]
fn probe_process(pid: u32) -> ProcessProbe {
    let Ok(output) = std::process::Command::new("ps")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .output()
    else {
        return ProcessProbe::Unknown;
    };
    if !output.status.success() {
        return if output.status.code() == Some(1) {
            ProcessProbe::Absent
        } else {
            ProcessProbe::Unknown
        };
    }
    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return ProcessProbe::Unknown;
    };
    let identity = stdout.trim().to_owned();
    if identity.is_empty() {
        ProcessProbe::Absent
    } else {
        ProcessProbe::Live(identity)
    }
}

pub fn process_start_identity(pid: u32) -> Option<String> {
    match probe_process(pid) {
        ProcessProbe::Live(identity) => Some(identity),
        ProcessProbe::Absent | ProcessProbe::Unknown => None,
    }
}

#[cfg(windows)]
fn probe_process(pid: u32) -> ProcessProbe {
    let query = format!(
        "$p = Get-Process -Id {pid} -ErrorAction SilentlyContinue; \
         if ($null -eq $p) {{ 'ABSENT'; exit }}; \
         try {{ $p.StartTime.ToUniversalTime().Ticks }} catch {{ 'UNKNOWN' }}"
    );
    let Ok(output) = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &query])
        .output()
    else {
        return ProcessProbe::Unknown;
    };
    if !output.status.success() {
        return ProcessProbe::Unknown;
    }
    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return ProcessProbe::Unknown;
    };
    match stdout.trim() {
        "ABSENT" => ProcessProbe::Absent,
        "UNKNOWN" | "" => ProcessProbe::Unknown,
        identity => ProcessProbe::Live(identity.to_owned()),
    }
}

#[cfg(not(any(unix, windows)))]
fn probe_process(_pid: u32) -> ProcessProbe {
    ProcessProbe::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_record(run_id: &str, status: &str) -> NewRunRecord {
        NewRunRecord {
            run_id: run_id.to_owned(),
            story_id: "US-STATE".to_owned(),
            branch: Some(format!("symphony/{run_id}")),
            worktree: PathBuf::from(format!(".symphony/worktrees/{run_id}")),
            lightweight: false,
            status: status.to_owned(),
            result_path: Some(PathBuf::from(format!(".harness/runs/{run_id}/RESULT.json"))),
            sync_status: "not_applicable".to_owned(),
            next_action: "continue run".to_owned(),
        }
    }

    #[test]
    fn store_creates_state_db_and_records_run() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));

        store.add_run(new_record("run_1", "prepared")).unwrap();

        assert!(temp_dir.path().join(".symphony/state.db").exists());
        let runs = store.list_runs().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, "run_1");
        assert_eq!(runs[0].story_id, "US-STATE");
    }

    #[test]
    fn runtime_control_transitions_are_durable() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store.add_run(new_record("run_1", "prepared")).unwrap();

        store
            .begin_execution("run_1", 4100, 4200, "agent-start", 1_721_000_000)
            .unwrap();
        let running = store.show_run("run_1").unwrap();
        assert_eq!(running.status, "running");
        assert_eq!(running.current_stage, "agent");
        assert_eq!(running.owner_pid, Some(4100));
        assert_eq!(running.agent_pid, Some(4200));
        assert!(!running.cancel_requested);

        store.request_cancel("run_1").unwrap();
        assert!(store.cancellation_requested("run_1").unwrap());
        store
            .finish_execution("run_1", "cancelled", "operator cancelled run")
            .unwrap();
        let cancelled = store.show_run("run_1").unwrap();
        assert_eq!(cancelled.status, "cancelled");
        assert_eq!(cancelled.owner_pid, None);
        assert_eq!(cancelled.agent_pid, None);
        assert_eq!(
            cancelled.terminal_reason.as_deref(),
            Some("operator cancelled run")
        );
    }

    #[test]
    fn second_active_run_is_refused() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));

        store.add_run(new_record("run_1", "prepared")).unwrap();
        let error = store.add_run(new_record("run_2", "prepared")).unwrap_err();

        assert!(matches!(error, StateError::ActiveRunExists(id) if id == "run_1"));
    }

    #[test]
    fn terminal_status_releases_active_lock() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));

        store.add_run(new_record("run_1", "prepared")).unwrap();
        store
            .update_status("run_1", "completed", "review result")
            .unwrap();
        store.add_run(new_record("run_2", "prepared")).unwrap();

        assert_eq!(store.active_run().unwrap().unwrap().run_id, "run_2");
    }

    #[test]
    fn request_changes_state_transition_is_atomic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));
        store.add_run(new_record("run_old", "completed")).unwrap();

        store
            .replace_run(
                "run_old",
                "Needs tighter spacing",
                new_record("run_new", "prepared"),
            )
            .unwrap();

        let source = store.show_run("run_old").unwrap();
        assert_eq!(source.status, "rejected");
        assert!(source.next_action.contains("Needs tighter spacing"));
        assert_eq!(store.active_run().unwrap().unwrap().run_id, "run_new");
    }

    #[test]
    fn request_changes_state_collision_rolls_back_source_rejection() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));
        store.add_run(new_record("run_old", "completed")).unwrap();

        let error = store
            .replace_run("run_old", "Try again", new_record("run_old", "prepared"))
            .unwrap_err();

        assert!(matches!(error, StateError::Sqlite(_)));
        assert_eq!(store.show_run("run_old").unwrap().status, "completed");
        assert!(store.active_run().unwrap().is_none());
    }

    #[test]
    fn request_changes_refuses_non_completed_source() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));
        store.add_run(new_record("run_old", "failed")).unwrap();

        let error = store
            .replace_run("run_old", "Try again", new_record("run_new", "prepared"))
            .unwrap_err();

        assert!(matches!(
            error,
            StateError::RunNotReplaceable { id, status }
                if id == "run_old" && status == "failed"
        ));
        assert!(matches!(
            store.show_run("run_new").unwrap_err(),
            StateError::RunNotFound(_)
        ));
    }

    #[test]
    fn request_changes_remove_run_releases_incomplete_replacement() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));
        store.add_run(new_record("run_new", "prepared")).unwrap();

        store.remove_run("run_new").unwrap();

        assert!(store.active_run().unwrap().is_none());
        assert!(matches!(
            store.show_run("run_new").unwrap_err(),
            StateError::RunNotFound(_)
        ));
    }

    #[test]
    fn failed_and_cancelled_release_active_lock() {
        for terminal in ["failed", "cancelled"] {
            let temp_dir = tempfile::tempdir().unwrap();
            let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));

            store.add_run(new_record("run_1", "running")).unwrap();
            store
                .update_status("run_1", terminal, "inspect result")
                .unwrap();

            assert!(store.active_run().unwrap().is_none());
        }
    }

    #[test]
    fn records_synced_changesets_idempotently() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));

        assert!(!store.changeset_synced("run_1").unwrap());
        store
            .record_changeset_synced(
                "run_1",
                std::path::Path::new(".harness/changesets/run_1.changeset.jsonl"),
                true,
            )
            .unwrap();
        store
            .record_changeset_synced(
                "run_1",
                std::path::Path::new(".harness/changesets/run_1.changeset.jsonl"),
                false,
            )
            .unwrap();

        assert!(store.changeset_synced("run_1").unwrap());
    }

    #[test]
    fn updates_run_sync_status() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));

        store.add_run(new_record("run_1", "completed")).unwrap();
        store.update_sync_status("run_1", "synced", "done").unwrap();

        let run = store.show_run("run_1").unwrap();
        assert_eq!(run.sync_status, "synced");
        assert_eq!(run.next_action, "done");
    }

    #[test]
    fn updates_pr_status() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));

        store.add_run(new_record("run_1", "completed")).unwrap();
        store
            .update_pr_url("run_1", "https://example.test/pr/1")
            .unwrap();
        assert_eq!(store.show_run("run_1").unwrap().pr_status, "created");

        store.update_pr_status("run_1", "merged").unwrap();

        let run = store.show_run("run_1").unwrap();
        assert_eq!(run.pr_status, "merged");
        assert_eq!(run.next_action, "approve sync");
    }

    #[test]
    fn records_pr_failure_without_changing_run_outcome() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));

        store.add_run(new_record("run_1", "completed")).unwrap();
        store.record_pr_failure("run_1", "gh auth failed").unwrap();

        let run = store.show_run("run_1").unwrap();
        assert_eq!(run.status, "completed");
        assert_eq!(run.pr_status, "failed");
        assert!(run.next_action.contains("gh auth failed"));
    }

    #[test]
    fn run_agent_defaults_to_codex_and_can_be_recorded() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));

        store.add_run(new_record("run_1", "prepared")).unwrap();
        assert_eq!(store.show_run("run_1").unwrap().agent, "codex");

        store.record_run_agent("run_1", "opencode").unwrap();
        assert_eq!(store.show_run("run_1").unwrap().agent, "opencode");

        let missing = store
            .record_run_agent("run_missing", "opencode")
            .unwrap_err();
        assert!(matches!(missing, StateError::RunNotFound(_)));
    }

    #[test]
    fn settings_round_trip_and_missing_key_is_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));

        assert_eq!(store.get_setting("default_agent").unwrap(), None);
        store.set_setting("default_agent", "opencode").unwrap();
        assert_eq!(
            store.get_setting("default_agent").unwrap(),
            Some("opencode".to_owned())
        );
        store.set_setting("default_agent", "codex").unwrap();
        assert_eq!(
            store.get_setting("default_agent").unwrap(),
            Some("codex".to_owned())
        );
    }

    #[test]
    fn queue_records_attempts_and_requeues_until_limit() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join(".symphony/state.db"));

        let queued = store.enqueue_work("US-AUTO", "harness-db", 2).unwrap();
        assert_eq!(queued.status, "queued");
        assert_eq!(queued.attempts, 0);

        let next = store
            .claim_next_queued_work_at(unix_timestamp(), "owner", 1, 60)
            .unwrap()
            .unwrap();
        assert_eq!(next.story_id, "US-AUTO");
        let failed_once = store
            .mark_queue_failed("US-AUTO", Some("run_1"), "agent failed", "owner")
            .unwrap();
        assert_eq!(failed_once.status, "queued");
        assert_eq!(failed_once.attempts, 1);

        store
            .claim_next_queued_work_at(unix_timestamp(), "owner", 1, 60)
            .unwrap();
        let failed_twice = store
            .mark_queue_failed("US-AUTO", Some("run_2"), "agent failed again", "owner")
            .unwrap();
        assert_eq!(failed_twice.status, "failed");
        assert_eq!(failed_twice.attempts, 2);
        assert!(store.next_queued_work().unwrap().is_none());
    }

    #[test]
    fn queue_migration_preserves_existing_rows_and_adds_recovery_columns() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("state.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE auto_queue (
                    story_id TEXT PRIMARY KEY, source TEXT NOT NULL, status TEXT NOT NULL,
                    attempts INTEGER NOT NULL DEFAULT 0, max_attempts INTEGER NOT NULL,
                    last_run_id TEXT, last_error TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                INSERT INTO auto_queue (story_id, source, status, max_attempts)
                VALUES ('US-OLD', 'harness-db', 'queued', 3);",
            )
            .unwrap();
        drop(connection);

        let store = RunStateStore::new(path);
        store.init().unwrap();

        let record = store.queue_record("US-OLD").unwrap();
        assert_eq!(record.status, "queued");
        assert_eq!(record.next_attempt_at, 0);
        assert_eq!(record.owner_token, None);
        assert_eq!(record.owner_start_identity, None);
    }

    #[test]
    fn concurrent_initializers_serialize_schema_migrations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("state.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE auto_queue (
                    story_id TEXT PRIMARY KEY, source TEXT NOT NULL, status TEXT NOT NULL,
                    attempts INTEGER NOT NULL DEFAULT 0, max_attempts INTEGER NOT NULL,
                    last_run_id TEXT, last_error TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );",
            )
            .unwrap();
        drop(connection);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let mut workers = Vec::new();
        for _ in 0..2 {
            let path = path.clone();
            let barrier = barrier.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                RunStateStore::new(path).init()
            }));
        }
        barrier.wait();

        for worker in workers {
            worker.join().unwrap().unwrap();
        }
        let record = RunStateStore::new(path)
            .enqueue_work("US-CONCURRENT", "harness-db", 1)
            .unwrap();
        assert_eq!(record.owner_start_identity, None);
    }

    #[test]
    fn atomic_claim_only_returns_due_work_and_records_owner_lease() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store.enqueue_work("US-DUE", "harness-db", 3).unwrap();

        let claimed = store
            .claim_next_queued_work_at(100, "owner-1", 4242, 60)
            .unwrap()
            .unwrap();
        assert_eq!(claimed.status, "running");
        assert_eq!(claimed.attempts, 1);
        assert_eq!(claimed.owner_token.as_deref(), Some("owner-1"));
        assert_eq!(claimed.owner_pid, Some(4242));
        assert_eq!(claimed.heartbeat_at, Some(100));
        assert_eq!(claimed.lease_expires_at, Some(160));
        assert!(store
            .claim_next_queued_work_at(100, "owner-2", 4243, 60)
            .unwrap()
            .is_none());
    }

    #[test]
    fn stale_worker_cannot_complete_or_fail_new_owners_claim() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store.enqueue_work("US-FENCED", "harness-db", 3).unwrap();
        store
            .claim_next_queued_work_at(100, "old-owner", 1, 10)
            .unwrap();
        Connection::open(&store.path)
            .unwrap()
            .execute(
                "UPDATE auto_queue SET owner_token='new-owner' WHERE story_id=?1",
                params!["US-FENCED"],
            )
            .unwrap();

        assert!(matches!(
            store.mark_queue_completed("US-FENCED", "run_old", "old-owner"),
            Err(StateError::QueueOwnershipLost(story)) if story == "US-FENCED"
        ));
        assert!(matches!(
            store.mark_queue_failed_at(
                "US-FENCED",
                Some("run_old"),
                "late failure",
                "old-owner",
                111,
                10,
                2,
                300,
            ),
            Err(StateError::QueueOwnershipLost(story)) if story == "US-FENCED"
        ));
        let queue = store.queue_record("US-FENCED").unwrap();
        assert_eq!(queue.status, "running");
        assert_eq!(queue.owner_token.as_deref(), Some("new-owner"));
        assert_eq!(queue.last_run_id, None);
        assert_eq!(queue.last_error, None);
    }

    #[test]
    fn failure_backoff_defers_selection_and_caps_exponentially() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store.enqueue_work("US-BACKOFF", "harness-db", 4).unwrap();

        store
            .claim_next_queued_work_at(100, "owner", 1, 60)
            .unwrap();
        let first = store
            .mark_queue_failed_at("US-BACKOFF", None, "failed", "owner", 100, 10, 2, 25)
            .unwrap();
        assert_eq!(first.next_attempt_at, 110);
        assert!(store
            .claim_next_queued_work_at(109, "owner", 1, 60)
            .unwrap()
            .is_none());

        store
            .claim_next_queued_work_at(110, "owner", 1, 60)
            .unwrap();
        let second = store
            .mark_queue_failed_at("US-BACKOFF", None, "failed", "owner", 110, 10, 2, 25)
            .unwrap();
        assert_eq!(second.next_attempt_at, 130);

        store
            .claim_next_queued_work_at(130, "owner", 1, 60)
            .unwrap();
        let third = store
            .mark_queue_failed_at("US-BACKOFF", None, "failed", "owner", 130, 10, 2, 25)
            .unwrap();
        assert_eq!(third.next_attempt_at, 155);
    }

    #[test]
    fn expired_orphan_interrupts_active_run_and_requeues_or_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store.enqueue_work("US-STATE", "harness-db", 2).unwrap();
        store
            .claim_next_queued_work_at(100, "dead-owner", std::process::id(), 10)
            .unwrap();
        store.add_run(new_record("run_orphan", "running")).unwrap();
        let recovered = store
            .recover_expired_work_at(111, "new-owner", std::process::id(), 10, 2, 300)
            .unwrap();
        assert_eq!(recovered, 1);
        assert_eq!(store.show_run("run_orphan").unwrap().status, "interrupted");
        let queue = store.queue_record("US-STATE").unwrap();
        assert_eq!(queue.status, "queued");
        assert_eq!(queue.next_attempt_at, 121);
        assert_eq!(queue.owner_token, None);

        store
            .claim_next_queued_work_at(121, "owner", std::process::id(), 10)
            .unwrap();
        let recovered = store
            .recover_expired_work_at(132, "new-owner", std::process::id(), 10, 2, 300)
            .unwrap();
        assert_eq!(recovered, 1);
        assert_eq!(store.queue_record("US-STATE").unwrap().status, "failed");
    }

    #[test]
    fn expired_lease_is_not_recovered_while_owner_process_is_alive() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store.enqueue_work("US-LIVE", "harness-db", 2).unwrap();
        store
            .claim_next_queued_work_at(100, "live-owner", std::process::id(), 10)
            .unwrap();

        let recovered = store
            .recover_expired_work_at(111, "live-owner", std::process::id(), 10, 2, 300)
            .unwrap();

        assert_eq!(recovered, 0);
        assert_eq!(store.queue_record("US-LIVE").unwrap().status, "running");
    }

    #[test]
    fn expired_lease_with_reused_current_pid_requires_matching_token() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store
            .enqueue_work("US-REUSED-PID", "harness-db", 2)
            .unwrap();
        store
            .claim_next_queued_work_at(100, "old-process-token", std::process::id(), 10)
            .unwrap();

        let recovered = store
            .recover_expired_work_at(111, "new-process-token", std::process::id(), 10, 2, 300)
            .unwrap();

        assert_eq!(recovered, 1);
        assert_eq!(
            store.queue_record("US-REUSED-PID").unwrap().status,
            "queued"
        );
    }

    #[test]
    fn expired_lease_with_reused_different_pid_requires_matching_start_identity() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store
            .enqueue_work("US-REUSED-OTHER", "harness-db", 2)
            .unwrap();
        store
            .claim_next_queued_work_at(100, "old-worker", 4242, 10)
            .unwrap();
        Connection::open(&store.path)
            .unwrap()
            .execute(
                "UPDATE auto_queue SET owner_start_identity='old-start' WHERE story_id=?1",
                params!["US-REUSED-OTHER"],
            )
            .unwrap();

        let recovered = store
            .recover_expired_work_at_with_identity(111, "new-worker", 4343, 10, 2, 300, |pid| {
                if pid == 4242 {
                    ProcessProbe::Live("new-start".to_owned())
                } else {
                    ProcessProbe::Unknown
                }
            })
            .unwrap();

        assert_eq!(recovered, 1);
        assert_eq!(
            store.queue_record("US-REUSED-OTHER").unwrap().status,
            "queued"
        );
    }

    #[test]
    fn unverifiable_different_owner_identity_fails_closed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store
            .enqueue_work("US-UNKNOWN-OWNER", "harness-db", 2)
            .unwrap();
        store
            .claim_next_queued_work_at(100, "other-worker", 4242, 10)
            .unwrap();

        let recovered = store
            .recover_expired_work_at_with_identity(111, "new-worker", 4343, 10, 2, 300, |_| {
                ProcessProbe::Unknown
            })
            .unwrap();

        assert_eq!(recovered, 0);
        assert_eq!(
            store.queue_record("US-UNKNOWN-OWNER").unwrap().status,
            "running"
        );
    }

    #[test]
    fn live_different_owner_without_recorded_identity_fails_closed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store
            .enqueue_work("US-LEGACY-OWNER", "harness-db", 2)
            .unwrap();
        store
            .claim_next_queued_work_at(100, "legacy-worker", 4242, 10)
            .unwrap();
        Connection::open(&store.path)
            .unwrap()
            .execute(
                "UPDATE auto_queue SET owner_start_identity=NULL WHERE story_id=?1",
                params!["US-LEGACY-OWNER"],
            )
            .unwrap();

        let recovered = store
            .recover_expired_work_at_with_identity(111, "new-worker", 4343, 10, 2, 300, |_| {
                ProcessProbe::Live("current-start".to_owned())
            })
            .unwrap();

        assert_eq!(recovered, 0);
        assert_eq!(
            store.queue_record("US-LEGACY-OWNER").unwrap().status,
            "running"
        );
    }

    #[test]
    fn expired_lease_with_absent_different_pid_is_recovered() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store
            .enqueue_work("US-DEAD-OWNER", "harness-db", 2)
            .unwrap();
        store
            .claim_next_queued_work_at(100, "dead-worker", 4242, 10)
            .unwrap();

        let recovered = store
            .recover_expired_work_at_with_identity(111, "new-worker", 4343, 10, 2, 300, |_| {
                ProcessProbe::Absent
            })
            .unwrap();

        assert_eq!(recovered, 1);
        assert_eq!(
            store.queue_record("US-DEAD-OWNER").unwrap().status,
            "queued"
        );
    }

    #[test]
    fn heartbeat_only_refreshes_matching_owner_lease() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = RunStateStore::new(temp_dir.path().join("state.db"));
        store.enqueue_work("US-HEART", "harness-db", 2).unwrap();
        store
            .claim_next_queued_work_at(100, "owner", 1, 10)
            .unwrap();

        assert!(!store
            .refresh_queue_lease_at("US-HEART", "stale-owner", 105, 10)
            .unwrap());
        assert!(store
            .refresh_queue_lease_at("US-HEART", "owner", 105, 10)
            .unwrap());
        assert_eq!(
            store.queue_record("US-HEART").unwrap().lease_expires_at,
            Some(115)
        );
        assert_eq!(
            store.queue_record("US-HEART").unwrap().heartbeat_at,
            Some(105)
        );
    }
}
