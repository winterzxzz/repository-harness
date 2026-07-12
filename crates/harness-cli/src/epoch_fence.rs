use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::Path;

use fs2::FileExt;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

const CONTROL_DIR: &str = ".harness/epoch-transition";

#[derive(Debug, Error)]
pub enum EpochFenceError {
    #[error("epoch transition control I/O failed at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("epoch transition journal is invalid at {path}: {reason}; writes remain fenced")]
    InvalidJournal { path: String, reason: String },
    #[error("epoch transition '{transition_id}' is {state}; writes remain fenced until the checksummed transition is completed or compensated")]
    TransitionInProgress {
        transition_id: String,
        state: String,
    },
}

#[derive(Debug, Deserialize)]
struct JournalEnvelope {
    payload: Value,
    payload_sha256: String,
}

/// A shared lock held for the complete lifetime of one state-mutating command.
/// The epoch transition utility takes the same file exclusively before it
/// creates or changes the journal, eliminating the check/start race.
pub struct EpochWriteGuard {
    lock: File,
}

impl Drop for EpochWriteGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.lock);
    }
}

pub fn acquire_command_guard(
    repo_root: &Path,
    mutates_state: bool,
) -> Result<EpochWriteGuard, EpochFenceError> {
    let control = repo_root.join(CONTROL_DIR);
    fs::create_dir_all(&control).map_err(|source| io_error(&control, source))?;
    let lock_path = control.join("writer.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|source| io_error(&lock_path, source))?;
    lock.lock_shared()
        .map_err(|source| io_error(&lock_path, source))?;

    let journal_path = control.join("journal.json");
    if journal_path.exists() {
        validate_journal(&journal_path, mutates_state)?;
    }
    Ok(EpochWriteGuard { lock })
}

fn validate_journal(path: &Path, mutates_state: bool) -> Result<(), EpochFenceError> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|source| io_error(path, source))?;
    let envelope: JournalEnvelope =
        serde_json::from_slice(&bytes).map_err(|error| EpochFenceError::InvalidJournal {
            path: display(path),
            reason: error.to_string(),
        })?;
    let canonical =
        serde_json::to_vec(&envelope.payload).map_err(|error| EpochFenceError::InvalidJournal {
            path: display(path),
            reason: error.to_string(),
        })?;
    let actual = format!("{:x}", Sha256::digest(canonical));
    if actual != envelope.payload_sha256 {
        return Err(EpochFenceError::InvalidJournal {
            path: display(path),
            reason: format!(
                "payload SHA-256 mismatch (declared {}, calculated {actual})",
                envelope.payload_sha256
            ),
        });
    }
    let state = envelope
        .payload
        .get("state")
        .and_then(Value::as_str)
        .ok_or_else(|| EpochFenceError::InvalidJournal {
            path: display(path),
            reason: "payload.state is missing or not a string".to_owned(),
        })?;
    let terminal = state == "complete" || state == "compensated";
    let verified_pair_pending_validation = state == "switched_pending_validation";
    if !terminal && (mutates_state || !verified_pair_pending_validation) {
        let transition_id = envelope
            .payload
            .get("transition_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        return Err(EpochFenceError::TransitionInProgress {
            transition_id,
            state: state.to_owned(),
        });
    }
    Ok(())
}

fn io_error(path: &Path, source: std::io::Error) -> EpochFenceError {
    EpochFenceError::Io {
        path: display(path),
        source,
    }
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_journal(root: &Path, state: &str, valid_hash: bool) {
        let dir = root.join(CONTROL_DIR);
        fs::create_dir_all(&dir).unwrap();
        let payload = serde_json::json!({
            "format_version": 1,
            "transition_id": "test-epoch",
            "state": state
        });
        let canonical = serde_json::to_vec(&payload).unwrap();
        let hash = if valid_hash {
            format!("{:x}", Sha256::digest(canonical))
        } else {
            "0".repeat(64)
        };
        let mut file = File::create(dir.join("journal.json")).unwrap();
        serde_json::to_writer(
            &mut file,
            &serde_json::json!({"payload": payload, "payload_sha256": hash}),
        )
        .unwrap();
        file.flush().unwrap();
    }

    #[test]
    fn incomplete_and_tampered_journals_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        write_journal(temp.path(), "prepared", true);
        assert!(matches!(
            acquire_command_guard(temp.path(), true),
            Err(EpochFenceError::TransitionInProgress { .. })
        ));
        write_journal(temp.path(), "complete", false);
        assert!(matches!(
            acquire_command_guard(temp.path(), true),
            Err(EpochFenceError::InvalidJournal { .. })
        ));
    }

    #[test]
    fn terminal_or_absent_journal_allows_writes() {
        let temp = tempfile::tempdir().unwrap();
        drop(acquire_command_guard(temp.path(), true).unwrap());
        write_journal(temp.path(), "complete", true);
        drop(acquire_command_guard(temp.path(), true).unwrap());
        write_journal(temp.path(), "compensated", true);
        drop(acquire_command_guard(temp.path(), true).unwrap());
    }

    #[test]
    fn switched_pair_allows_reads_but_not_writes() {
        let temp = tempfile::tempdir().unwrap();
        write_journal(temp.path(), "switched_pending_validation", true);
        drop(acquire_command_guard(temp.path(), false).unwrap());
        assert!(matches!(
            acquire_command_guard(temp.path(), true),
            Err(EpochFenceError::TransitionInProgress { .. })
        ));
    }
}
