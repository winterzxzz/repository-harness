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

    pub fn list_runs(&self) -> Result<Vec<RunRecord>, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        let mut statement = connection.prepare(
            "SELECT run_id, story_id, branch, worktree, lightweight, status, result_path,
                    pr_url, pr_status, sync_status, next_action, agent
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
                        pr_url, pr_status, sync_status, next_action, agent
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
                        pr_url, pr_status, sync_status, next_action, agent
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

    pub fn next_queued_work(&self) -> Result<Option<QueueRecord>, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection
            .query_row(
                "SELECT story_id, source, status, attempts, max_attempts, last_run_id, last_error
                 FROM auto_queue
                 WHERE status='queued' AND attempts < max_attempts
                 ORDER BY created_at ASC, story_id ASC
                 LIMIT 1;",
                [],
                queue_from_row,
            )
            .optional()
            .map_err(StateError::from)
    }

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

    pub fn mark_queue_completed(&self, story_id: &str, run_id: &str) -> Result<(), StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE auto_queue
             SET status='completed', last_run_id=?2, last_error=NULL, updated_at=datetime('now')
             WHERE story_id=?1;",
            params![story_id, run_id],
        )?;
        Ok(())
    }

    pub fn mark_queue_failed(
        &self,
        story_id: &str,
        run_id: Option<&str>,
        error: &str,
    ) -> Result<QueueRecord, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection.execute(
            "UPDATE auto_queue
             SET status=CASE WHEN attempts >= max_attempts THEN 'failed' ELSE 'queued' END,
                 last_run_id=?2,
                 last_error=?3,
                 updated_at=datetime('now')
             WHERE story_id=?1;",
            params![story_id, run_id, error],
        )?;
        self.queue_record(story_id)
    }

    pub fn queue_record(&self, story_id: &str) -> Result<QueueRecord, StateError> {
        self.init()?;
        let connection = Connection::open(&self.path)?;
        connection
            .query_row(
                "SELECT story_id, source, status, attempts, max_attempts, last_run_id, last_error
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
    })
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

        let next = store.next_queued_work().unwrap().unwrap();
        assert_eq!(next.story_id, "US-AUTO");
        store.mark_queue_running("US-AUTO").unwrap();
        let failed_once = store
            .mark_queue_failed("US-AUTO", Some("run_1"), "agent failed")
            .unwrap();
        assert_eq!(failed_once.status, "queued");
        assert_eq!(failed_once.attempts, 1);

        store.mark_queue_running("US-AUTO").unwrap();
        let failed_twice = store
            .mark_queue_failed("US-AUTO", Some("run_2"), "agent failed again")
            .unwrap();
        assert_eq!(failed_twice.status, "failed");
        assert_eq!(failed_twice.attempts, 2);
        assert!(store.next_queued_work().unwrap().is_none());
    }
}
