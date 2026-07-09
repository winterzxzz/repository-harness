use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::BenchError;

/// Metadata describing one captured run, read from `meta.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Meta {
    pub task: String,
    /// "h0" (bare) or "hn" (harnessed).
    pub arm: String,
    pub k: u32,
    pub agent: String,
    pub exit_code: i32,
}

/// A captured run on disk: produced worktree + optional harness database.
#[derive(Debug, Clone, PartialEq)]
pub struct Artifact {
    pub dir: PathBuf,
    pub meta: Meta,
    pub worktree: PathBuf,
    pub harness_db: Option<PathBuf>,
}

impl Artifact {
    /// Load an artifact directory. Requires `meta.json` and a `worktree/`
    /// directory; `harness.db` is optional (absent on the bare arm).
    pub fn load(dir: &Path) -> Result<Self, BenchError> {
        let meta_path = dir.join("meta.json");
        let meta_text = std::fs::read_to_string(&meta_path).map_err(|source| BenchError::Io {
            path: meta_path.display().to_string(),
            source,
        })?;
        let meta: Meta =
            serde_json::from_str(&meta_text).map_err(|e| BenchError::MetaParse(e.to_string()))?;

        let worktree = dir.join("worktree");
        if !worktree.is_dir() {
            return Err(BenchError::ArtifactMissing(worktree.display().to_string()));
        }

        let db = dir.join("harness.db");
        let harness_db = if db.is_file() { Some(db) } else { None };

        Ok(Artifact {
            dir: dir.to_path_buf(),
            meta,
            worktree,
            harness_db,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_meta(dir: &Path, arm: &str) {
        let meta = format!(r#"{{"task":"T1","arm":"{arm}","k":1,"agent":"fake","exit_code":0}}"#);
        fs::write(dir.join("meta.json"), meta).unwrap();
        fs::create_dir_all(dir.join("worktree")).unwrap();
    }

    #[test]
    fn loads_hn_artifact_with_db() {
        let tmp = tempfile::tempdir().unwrap();
        write_meta(tmp.path(), "hn");
        fs::write(tmp.path().join("harness.db"), b"").unwrap();

        let artifact = Artifact::load(tmp.path()).unwrap();
        assert_eq!(artifact.meta.arm, "hn");
        assert_eq!(artifact.meta.task, "T1");
        assert_eq!(artifact.worktree, tmp.path().join("worktree"));
        assert_eq!(artifact.harness_db, Some(tmp.path().join("harness.db")));
    }

    #[test]
    fn bare_artifact_has_no_db() {
        let tmp = tempfile::tempdir().unwrap();
        write_meta(tmp.path(), "h0");
        let artifact = Artifact::load(tmp.path()).unwrap();
        assert!(artifact.harness_db.is_none());
    }

    #[test]
    fn missing_worktree_errors() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("meta.json"),
            r#"{"task":"T1","arm":"h0","k":1,"agent":"fake","exit_code":0}"#,
        )
        .unwrap();
        let err = Artifact::load(tmp.path()).unwrap_err();
        assert!(matches!(err, BenchError::ArtifactMissing(_)));
    }
}
