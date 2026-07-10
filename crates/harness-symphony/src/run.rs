use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use crate::agent::{run_agent, AgentError};
use crate::changeset::{append_rendered_section, ChangesetError};
use crate::config::ResolvedConfig;
use crate::state::{NewRunRecord, RunStateStore, StateError};
use crate::sync::SyncError;

#[derive(Debug, Error)]
pub enum RunError {
    #[error("story {0} not found in harness database")]
    StoryNotFound(String),
    #[error("story {id} is not runnable because status is {status}; only planned or in_progress can be prepared")]
    StoryNotRunnable { id: String, status: String },
    #[error("story {id} cannot use --here because lane is {lane}; only tiny stories may run in the current checkout")]
    StoryNotTiny { id: String, lane: String },
    #[error("--here is disabled by config. Set runs.allow_here_for_tiny: true in .harness/symphony.yml.")]
    HereRunDisabled,
    #[error("harness database not found at {0}. Run: scripts/bin/harness-cli init")]
    MissingDatabase(String),
    #[error("git worktree failed: {0}")]
    GitWorktree(String),
    #[error("run result validation failed: {0}")]
    InvalidResult(String),
    #[error("request changes feedback is invalid: {0}")]
    InvalidFeedback(String),
    #[error("{0}")]
    Agent(#[from] AgentError),
    #[error("{0}")]
    State(#[from] StateError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Changeset(#[from] ChangesetError),
    #[error("{0}")]
    Sync(#[from] SyncError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRun {
    pub run_id: String,
    pub story_id: String,
    pub branch: Option<String>,
    pub worktree: PathBuf,
    pub contract_path: PathBuf,
    pub harness_db_path: PathBuf,
    pub lightweight: bool,
    pub request_changes: Option<RequestChangesContract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedRun {
    pub prepared: PreparedRun,
    pub outcome: String,
    pub summary_path: PathBuf,
    pub result_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RunContract {
    pub version: u32,
    pub run_id: String,
    pub mode: String,
    pub story_id: String,
    pub lightweight: bool,
    pub worktree: String,
    pub harness_db_path: String,
    pub env: RunEnvironment,
    pub required_outputs: Vec<String>,
    pub result_json_schema: Value,
    pub forbidden_paths: Vec<String>,
    pub agent_instructions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_changes: Option<RequestChangesContract>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RunEnvironment {
    pub harness_db_path: String,
    pub harness_run_id: String,
    pub harness_run_mode: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RequestChangesContract {
    pub source_run_id: String,
    pub reason_path: String,
    pub evidence_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementFeedback {
    pub source_run_id: String,
    pub reason: String,
    pub evidence: Vec<FeedbackFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackFile {
    pub extension: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct ResultFile {
    version: u32,
    run_id: String,
    story_id: String,
    outcome: String,
    validation: Option<ResultValidation>,
    summary_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResultValidation {
    commands: Option<Vec<ValidationCommand>>,
    unavailable: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ValidationCommand {
    command: String,
    result: String,
}

// Prepare intentionally does not pull from upstream: mutating the user's
// checkout is a surprising side effect, and a dirty checkout would make every
// run fail. Runs branch from the current HEAD; pull first if you want latest.
pub fn prepare_run(config: &ResolvedConfig, story_id: &str) -> Result<PreparedRun, RunError> {
    ensure_no_active_run(config)?;
    let story = load_runnable_story(&config.harness_db, story_id)?;

    let run_id = generate_run_id();
    let branch = format!("symphony/{run_id}");
    let worktree = config.worktrees_dir.join(&run_id);
    let run_dir = config.runs_dir.join(&run_id);
    let contract_path = run_dir.join("RUN_CONTRACT.json");
    let harness_db_path = worktree.join("harness.db");

    fs::create_dir_all(&config.worktrees_dir)?;
    fs::create_dir_all(&run_dir)?;
    create_worktree(&config.repo_root, &branch, &worktree)?;
    fs::copy(&config.harness_db, &harness_db_path)?;

    let contract = build_contract(
        config,
        &run_id,
        story_id,
        false,
        &worktree,
        &harness_db_path,
    );
    write_contract(&contract_path, &contract)?;
    // Also place the contract inside the worktree so the agent never has to
    // reach outside its assigned workspace to read its own run contract.
    let worktree_contract_relative = format!(".harness/runs/{run_id}/RUN_CONTRACT.json");
    let worktree_contract_path = worktree.join(&worktree_contract_relative);
    if let Some(parent) = worktree_contract_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_contract(&worktree_contract_path, &contract)?;
    write_agents_shim(
        &worktree.join("AGENTS.md"),
        Path::new(&worktree_contract_relative),
        &contract,
    )?;

    let store = RunStateStore::new(config.state_db.clone());
    store.add_run(NewRunRecord {
        run_id: run_id.clone(),
        story_id: story.id,
        branch: Some(branch.clone()),
        worktree: worktree.clone(),
        lightweight: false,
        status: "prepared".to_owned(),
        result_path: Some(PathBuf::from(format!(".harness/runs/{run_id}/RESULT.json"))),
        sync_status: "not_applied".to_owned(),
        next_action: format!("Launch agent for {story_id} or inspect {contract_path:?}"),
    })?;
    store.record_run_agent(&run_id, &config.agent_adapter)?;

    Ok(PreparedRun {
        run_id,
        story_id: story_id.to_owned(),
        branch: Some(branch),
        worktree,
        contract_path,
        harness_db_path,
        lightweight: false,
        request_changes: None,
    })
}

pub fn prepare_replacement_run(
    config: &ResolvedConfig,
    story_id: &str,
    feedback: ReplacementFeedback,
) -> Result<PreparedRun, RunError> {
    let source_run_id = feedback.source_run_id.clone();
    let rejection_reason = feedback.reason.clone();
    let prepared = prepare_replacement_files(config, story_id, feedback)?;
    finalize_replacement_run(config, &source_run_id, &rejection_reason, prepared)
}

fn prepare_replacement_files(
    config: &ResolvedConfig,
    story_id: &str,
    mut feedback: ReplacementFeedback,
) -> Result<PreparedRun, RunError> {
    validate_replacement_feedback(&mut feedback)?;
    ensure_no_active_run(config)?;
    let store = RunStateStore::new(config.state_db.clone());
    let source = store.show_run(&feedback.source_run_id)?;
    if source.status != "completed" {
        return Err(StateError::RunNotReplaceable {
            id: source.run_id,
            status: source.status,
        }
        .into());
    }
    if source.story_id != story_id {
        return Err(StateError::ReplacementStoryMismatch {
            source_run_id: source.run_id,
            source_story_id: source.story_id,
            replacement_story_id: story_id.to_owned(),
        }
        .into());
    }

    // Like prepare_run, replacement prepare does not pull from upstream; the
    // replacement branches from the current HEAD.
    let story = load_runnable_story(&config.harness_db, story_id)?;
    let run_id = generate_run_id();
    let branch = format!("symphony/{run_id}");
    let worktree = config.worktrees_dir.join(&run_id);
    let run_dir = config.runs_dir.join(&run_id);
    let contract_path = run_dir.join("RUN_CONTRACT.json");
    let harness_db_path = worktree.join("harness.db");
    let request_changes = request_changes_contract(&run_id, &feedback);
    let prepared = PreparedRun {
        run_id: run_id.clone(),
        story_id: story.id,
        branch: Some(branch.clone()),
        worktree: worktree.clone(),
        contract_path: contract_path.clone(),
        harness_db_path: harness_db_path.clone(),
        lightweight: false,
        request_changes: Some(request_changes.clone()),
    };

    let preparation = (|| -> Result<(), RunError> {
        fs::create_dir_all(&config.worktrees_dir)?;
        fs::create_dir_all(&run_dir)?;
        create_worktree(&config.repo_root, &branch, &worktree)?;
        fs::copy(&config.harness_db, &harness_db_path)?;

        let worktree_run_dir = worktree.join(".harness/runs").join(&run_id);
        fs::create_dir_all(&worktree_run_dir)?;
        write_feedback_directory(&run_dir, &feedback)?;
        write_feedback_directory(&worktree_run_dir, &feedback)?;

        let mut contract = build_contract(
            config,
            &run_id,
            story_id,
            false,
            &worktree,
            &harness_db_path,
        );
        contract.request_changes = Some(request_changes);
        write_contract(&contract_path, &contract)?;
        // Mirror prepare_run: contract copy inside the worktree, shim pointing
        // at the worktree-relative path.
        let worktree_contract_relative = format!(".harness/runs/{run_id}/RUN_CONTRACT.json");
        write_contract(&worktree.join(&worktree_contract_relative), &contract)?;
        write_agents_shim(
            &worktree.join("AGENTS.md"),
            Path::new(&worktree_contract_relative),
            &contract,
        )?;
        Ok(())
    })();
    if let Err(error) = preparation {
        cleanup_replacement_files(config, &prepared);
        return Err(error);
    }
    Ok(prepared)
}

fn finalize_replacement_run(
    config: &ResolvedConfig,
    source_run_id: &str,
    rejection_reason: &str,
    prepared: PreparedRun,
) -> Result<PreparedRun, RunError> {
    let replacement = NewRunRecord {
        run_id: prepared.run_id.clone(),
        story_id: prepared.story_id.clone(),
        branch: prepared.branch.clone(),
        worktree: prepared.worktree.clone(),
        lightweight: false,
        status: "prepared".to_owned(),
        result_path: Some(PathBuf::from(format!(
            ".harness/runs/{}/RESULT.json",
            prepared.run_id
        ))),
        sync_status: "not_applied".to_owned(),
        next_action: format!(
            "Launch replacement agent for {} or inspect {:?}",
            prepared.story_id, prepared.contract_path
        ),
    };
    let store = RunStateStore::new(config.state_db.clone());
    if let Err(error) = store.replace_run_with_agent(
        source_run_id,
        rejection_reason,
        replacement,
        &config.agent_adapter,
    ) {
        cleanup_replacement_files(config, &prepared);
        return Err(error.into());
    }
    Ok(prepared)
}

fn validate_replacement_feedback(feedback: &mut ReplacementFeedback) -> Result<(), RunError> {
    feedback.reason = feedback.reason.trim().to_owned();
    let reason_chars = feedback.reason.chars().count();
    if reason_chars == 0 || reason_chars > crate::upload::MAX_REASON_CHARS {
        return Err(RunError::InvalidFeedback(
            "reason must be 1-2000 characters".to_owned(),
        ));
    }
    if feedback.evidence.len() > crate::upload::MAX_EVIDENCE_FILES {
        return Err(RunError::InvalidFeedback(
            "at most 3 evidence images are allowed".to_owned(),
        ));
    }
    for file in &feedback.evidence {
        if !matches!(file.extension.as_str(), "png" | "jpg" | "webp") {
            return Err(RunError::InvalidFeedback(
                "evidence extension must be png, jpg, or webp".to_owned(),
            ));
        }
        if file.bytes.is_empty() || file.bytes.len() > crate::upload::MAX_EVIDENCE_BYTES {
            return Err(RunError::InvalidFeedback(
                "evidence image must be 1 byte to 5 MB".to_owned(),
            ));
        }
    }
    Ok(())
}

fn request_changes_contract(
    run_id: &str,
    feedback: &ReplacementFeedback,
) -> RequestChangesContract {
    RequestChangesContract {
        source_run_id: feedback.source_run_id.clone(),
        reason_path: format!(".harness/runs/{run_id}/feedback/reason.md"),
        evidence_paths: feedback
            .evidence
            .iter()
            .enumerate()
            .map(|(index, file)| {
                format!(
                    ".harness/runs/{run_id}/feedback/evidence-{:02}.{}",
                    index + 1,
                    file.extension
                )
            })
            .collect(),
    }
}

fn write_feedback_directory(
    run_dir: &Path,
    feedback: &ReplacementFeedback,
) -> Result<(), RunError> {
    let staging = run_dir.join("feedback.staging");
    let destination = run_dir.join("feedback");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;
    fs::write(staging.join("reason.md"), format!("{}\n", feedback.reason))?;
    for (index, file) in feedback.evidence.iter().enumerate() {
        fs::write(
            staging.join(format!("evidence-{:02}.{}", index + 1, file.extension)),
            &file.bytes,
        )?;
    }
    fs::rename(staging, destination)?;
    Ok(())
}

fn cleanup_replacement_files(config: &ResolvedConfig, prepared: &PreparedRun) {
    if let Some(branch) = prepared.branch.as_deref() {
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&prepared.worktree)
            .current_dir(&config.repo_root)
            .output();
        if prepared.worktree.exists() {
            let _ = fs::remove_dir_all(&prepared.worktree);
        }
        let _ = Command::new("git")
            .args(["branch", "-D", branch])
            .current_dir(&config.repo_root)
            .output();
    }
    let run_dir = config.runs_dir.join(&prepared.run_id);
    if run_dir.exists() {
        let _ = fs::remove_dir_all(run_dir);
    }
}

pub fn prepare_here_run(config: &ResolvedConfig, story_id: &str) -> Result<PreparedRun, RunError> {
    if !config.allow_here_for_tiny {
        return Err(RunError::HereRunDisabled);
    }
    ensure_no_active_run(config)?;
    let story = load_runnable_story(&config.harness_db, story_id)?;
    if story.lane != "tiny" {
        return Err(RunError::StoryNotTiny {
            id: story.id,
            lane: story.lane,
        });
    }

    let run_id = generate_run_id();
    let run_dir = config.runs_dir.join(&run_id);
    let contract_path = run_dir.join("RUN_CONTRACT.json");
    let local_run_dir = config.repo_root.join(".symphony/runs").join(&run_id);
    let harness_db_path = local_run_dir.join("harness.db");

    fs::create_dir_all(&run_dir)?;
    fs::create_dir_all(&local_run_dir)?;
    fs::copy(&config.harness_db, &harness_db_path)?;

    let contract = build_contract(
        config,
        &run_id,
        story_id,
        true,
        &config.repo_root,
        &harness_db_path,
    );
    write_contract(&contract_path, &contract)?;

    let store = RunStateStore::new(config.state_db.clone());
    store.add_run(NewRunRecord {
        run_id: run_id.clone(),
        story_id: story_id.to_owned(),
        branch: None,
        worktree: config.repo_root.clone(),
        lightweight: true,
        status: "prepared".to_owned(),
        result_path: Some(PathBuf::from(format!(".harness/runs/{run_id}/RESULT.json"))),
        sync_status: "not_applied".to_owned(),
        next_action: format!("Launch lightweight run for {story_id} or inspect {contract_path:?}"),
    })?;
    store.record_run_agent(&run_id, &config.agent_adapter)?;

    Ok(PreparedRun {
        run_id,
        story_id: story_id.to_owned(),
        branch: None,
        worktree: config.repo_root.clone(),
        contract_path,
        harness_db_path,
        lightweight: true,
        request_changes: None,
    })
}

pub fn execute_run(config: &ResolvedConfig, story_id: &str) -> Result<CompletedRun, RunError> {
    execute_prepared_run(config, prepare_run(config, story_id)?)
}

pub fn execute_here_run(config: &ResolvedConfig, story_id: &str) -> Result<CompletedRun, RunError> {
    execute_prepared_run(config, prepare_here_run(config, story_id)?)
}

pub fn execute_prepared_run(
    config: &ResolvedConfig,
    prepared: PreparedRun,
) -> Result<CompletedRun, RunError> {
    if let Err(error) = run_agent(config, &prepared) {
        RunStateStore::new(config.state_db.clone()).update_status(
            &prepared.run_id,
            "failed",
            "inspect agent command failure",
        )?;
        return Err(error.into());
    }

    let run_id = prepared.run_id.clone();
    let completed = match validate_finished_run(config, prepared) {
        Ok(completed) => completed,
        Err(error) => {
            RunStateStore::new(config.state_db.clone()).update_status(
                &run_id,
                "failed",
                "inspect invalid run result",
            )?;
            return Err(error);
        }
    };
    RunStateStore::new(config.state_db.clone()).update_status(
        &completed.prepared.run_id,
        &completed.outcome,
        "review run result",
    )?;
    Ok(completed)
}

fn load_runnable_story(db_path: &Path, story_id: &str) -> Result<Story, RunError> {
    let story = load_story(db_path, story_id)?;
    if !matches!(story.status.as_str(), "planned" | "in_progress") {
        return Err(RunError::StoryNotRunnable {
            id: story.id,
            status: story.status,
        });
    }
    Ok(story)
}

fn ensure_no_active_run(config: &ResolvedConfig) -> Result<(), RunError> {
    if let Some(active) = RunStateStore::new(config.state_db.clone()).active_run()? {
        return Err(StateError::ActiveRunExists(active.run_id).into());
    }
    Ok(())
}

fn load_story(db_path: &Path, story_id: &str) -> Result<Story, RunError> {
    if !db_path.exists() {
        return Err(RunError::MissingDatabase(db_path.display().to_string()));
    }
    let connection = Connection::open(db_path)?;
    connection
        .query_row(
            "SELECT id, status, risk_lane FROM story WHERE id=?1;",
            params![story_id],
            |row| {
                Ok(Story {
                    id: row.get(0)?,
                    status: row.get(1)?,
                    lane: row.get(2)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| RunError::StoryNotFound(story_id.to_owned()))
}

fn create_worktree(repo_root: &Path, branch: &str, worktree: &Path) -> Result<(), RunError> {
    let output = Command::new("git")
        .args(["worktree", "add", "-b", branch])
        .arg(worktree)
        .arg("HEAD")
        .current_dir(repo_root)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(RunError::GitWorktree(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

fn build_contract(
    config: &ResolvedConfig,
    run_id: &str,
    story_id: &str,
    lightweight: bool,
    worktree: &Path,
    harness_db_path: &Path,
) -> RunContract {
    let required_outputs = vec![
        format!(".harness/runs/{run_id}/SUMMARY.md"),
        format!(".harness/runs/{run_id}/RESULT.json"),
    ];
    let forbidden_paths = vec![
        "harness.db".to_owned(),
        ".symphony/state.db".to_owned(),
        ".symphony/runs/**".to_owned(),
        ".symphony/worktrees/**".to_owned(),
    ];
    let result_json_schema = json!({
        "version": 1,
        "run_id": run_id,
        "story_id": story_id,
        "outcome": "completed | blocked | needs_intake | partial | failed | cancelled",
        "summary_path": format!(".harness/runs/{run_id}/SUMMARY.md"),
        "validation": {
            "commands": [
                {
                    "command": "exact validation command",
                    "result": "pass | fail | unavailable"
                }
            ],
            "unavailable": "optional non-empty reason when commands cannot be run"
        }
    });
    RunContract {
        version: 1,
        run_id: run_id.to_owned(),
        mode: "execute".to_owned(),
        story_id: story_id.to_owned(),
        lightweight,
        worktree: display_path(config, worktree),
        harness_db_path: display_path(config, harness_db_path),
        env: RunEnvironment {
            harness_db_path: display_path(config, harness_db_path),
            harness_run_id: run_id.to_owned(),
            harness_run_mode: "execute".to_owned(),
        },
        required_outputs,
        result_json_schema,
        forbidden_paths,
        agent_instructions: vec![
            "Follow AGENTS.md and Harness docs.".to_owned(),
            "Implement only the assigned story scope.".to_owned(),
            "Use the copied harness.db through HARNESS_DB_PATH; forbidden_paths lists files that must never be committed, and does not forbid Harness CLI writes to that database.".to_owned(),
            "Run the configured verification command when available.".to_owned(),
            "Write RESULT.json with a top-level validation object, not validation_evidence. Use validation.commands[].result values pass, fail, or unavailable.".to_owned(),
        ],
        request_changes: None,
    }
}

fn write_contract(path: &Path, contract: &RunContract) -> Result<(), RunError> {
    let text = serde_json::to_string_pretty(contract)?;
    fs::write(path, format!("{text}\n"))?;
    Ok(())
}

fn write_agents_shim(
    path: &Path,
    contract_path: &Path,
    contract: &RunContract,
) -> Result<(), RunError> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let block = render_agents_shim(contract_path, contract);
    fs::write(path, format!("{existing}\n{block}"))?;
    Ok(())
}

fn validate_finished_run(
    config: &ResolvedConfig,
    prepared: PreparedRun,
) -> Result<CompletedRun, RunError> {
    let summary_path = prepared
        .worktree
        .join(format!(".harness/runs/{}/SUMMARY.md", prepared.run_id));
    let result_path = prepared
        .worktree
        .join(format!(".harness/runs/{}/RESULT.json", prepared.run_id));

    if !summary_path.exists() {
        return Err(RunError::InvalidResult(format!(
            "SUMMARY.md missing at {}",
            summary_path.display()
        )));
    }
    if !result_path.exists() {
        return Err(RunError::InvalidResult(format!(
            "RESULT.json missing at {}",
            result_path.display()
        )));
    }
    promote_run_artifacts(config, &prepared, &summary_path, &result_path)?;

    let result = parse_result_file(&result_path)?;
    if result.version != 1 {
        return Err(RunError::InvalidResult(
            "RESULT.json version must be 1".to_owned(),
        ));
    }
    if result.run_id != prepared.run_id {
        return Err(RunError::InvalidResult(
            "RESULT.json run_id mismatch".to_owned(),
        ));
    }
    if result.story_id != prepared.story_id {
        return Err(RunError::InvalidResult(
            "RESULT.json story_id mismatch".to_owned(),
        ));
    }
    if !valid_outcome(&result.outcome) {
        return Err(RunError::InvalidResult(format!(
            "invalid outcome '{}'",
            result.outcome
        )));
    }
    if !has_validation_evidence(result.validation.as_ref()) {
        return Err(RunError::InvalidResult(
            "validation evidence missing or unavailable reason absent".to_owned(),
        ));
    }
    if let Some(summary) = result.summary_path.as_deref() {
        if summary.trim().is_empty() {
            return Err(RunError::InvalidResult(
                "summary_path must not be empty".to_owned(),
            ));
        }
    }
    ensure_forbidden_paths_not_staged(config, &prepared.worktree)?;
    if prepared.lightweight {
        let changeset_path = prepared.worktree.join(format!(
            ".harness/changesets/{}.changeset.jsonl",
            prepared.run_id
        ));
        if !changeset_path.exists() {
            return Err(RunError::InvalidResult(format!(
                "operation log missing at {}",
                changeset_path.display()
            )));
        }
        append_lightweight_summary_marker(&summary_path)?;
    }
    if config.changeset_render_in_summary {
        let changeset_path = prepared.worktree.join(format!(
            ".harness/changesets/{}.changeset.jsonl",
            prepared.run_id
        ));
        if changeset_path.exists() {
            append_rendered_section(
                &summary_path,
                &changeset_path,
                &format!(".harness/changesets/{}.changeset.jsonl", prepared.run_id),
            )?;
        }
    }

    let (summary_path, result_path) =
        promote_run_artifacts(config, &prepared, &summary_path, &result_path)?;

    Ok(CompletedRun {
        prepared,
        outcome: result.outcome,
        summary_path,
        result_path,
    })
}

fn promote_run_artifacts(
    config: &ResolvedConfig,
    prepared: &PreparedRun,
    summary_path: &Path,
    result_path: &Path,
) -> Result<(PathBuf, PathBuf), RunError> {
    if prepared.lightweight {
        return Ok((summary_path.to_path_buf(), result_path.to_path_buf()));
    }

    let run_dir = config.runs_dir.join(&prepared.run_id);
    fs::create_dir_all(&run_dir)?;
    let promoted_summary = run_dir.join("SUMMARY.md");
    let promoted_result = run_dir.join("RESULT.json");
    copy_if_different(summary_path, &promoted_summary)?;
    copy_if_different(result_path, &promoted_result)?;

    let changeset_path = prepared.worktree.join(format!(
        ".harness/changesets/{}.changeset.jsonl",
        prepared.run_id
    ));
    if changeset_path.exists() {
        copy_if_different(&changeset_path, &run_dir.join("changeset.jsonl"))?;
    }

    Ok((promoted_summary, promoted_result))
}

fn copy_if_different(source: &Path, destination: &Path) -> Result<(), RunError> {
    if source == destination {
        return Ok(());
    }
    fs::copy(source, destination)?;
    Ok(())
}

fn append_lightweight_summary_marker(summary_path: &Path) -> Result<(), RunError> {
    use std::io::Write;

    let mut file = fs::OpenOptions::new().append(true).open(summary_path)?;
    writeln!(file, "\n## Run Mode\n\nlightweight: true")?;
    Ok(())
}

fn parse_result_file(path: &Path) -> Result<ResultFile, RunError> {
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(RunError::Json)
}

fn valid_outcome(value: &str) -> bool {
    matches!(
        value,
        "completed" | "blocked" | "needs_intake" | "partial" | "failed" | "cancelled"
    )
}

fn has_validation_evidence(validation: Option<&ResultValidation>) -> bool {
    let Some(validation) = validation else {
        return false;
    };
    if validation
        .commands
        .as_ref()
        .is_some_and(|commands| !commands.is_empty() && commands.iter().all(valid_command))
    {
        return true;
    }
    validation
        .unavailable
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn valid_command(command: &ValidationCommand) -> bool {
    !command.command.trim().is_empty()
        && matches!(command.result.as_str(), "pass" | "fail" | "unavailable")
}

fn ensure_forbidden_paths_not_staged(
    _config: &ResolvedConfig,
    worktree: &Path,
) -> Result<(), RunError> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(worktree)
        .output()?;
    if !output.status.success() {
        return Err(RunError::GitWorktree(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let forbidden = ["harness.db", ".symphony/state.db"];
    let staged = String::from_utf8_lossy(&output.stdout);
    for path in staged
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if forbidden.contains(&path)
            || path.starts_with(".symphony/runs/")
            || path.starts_with(".symphony/worktrees/")
        {
            return Err(RunError::InvalidResult(format!(
                "forbidden path staged for commit: {path}"
            )));
        }
    }
    Ok(())
}

fn render_agents_shim(contract_path: &Path, contract: &RunContract) -> String {
    format!(
        "<!-- HARNESS-SYMPHONY:BEGIN -->\n\
## Harness Symphony Run\n\n\
- Story: `{}`\n\
- Contract: `{}`\n\
- Harness DB: `{}`\n\
- Required outputs: `{}` and `{}`\n\
- RESULT.json schema: `{}`\n\
- Forbidden to commit (writing the copied DB via HARNESS_DB_PATH is allowed): `{}`\n\
\n\
Use `HARNESS_DB_PATH={}`, `HARNESS_RUN_ID={}`, and `HARNESS_RUN_MODE=execute` for Harness CLI writes.\n\
<!-- HARNESS-SYMPHONY:END -->\n",
        contract.story_id,
        contract_path.display(),
        contract.harness_db_path,
        contract.required_outputs[0],
        contract.required_outputs[1],
        contract.result_json_schema,
        contract.forbidden_paths.join("`, `"),
        contract.env.harness_db_path,
        contract.env.harness_run_id,
    )
}

fn display_path(config: &ResolvedConfig, path: &Path) -> String {
    let relative = path
        .strip_prefix(&config.repo_root)
        .unwrap_or(path)
        .display()
        .to_string();
    if relative.is_empty() {
        ".".to_owned()
    } else {
        relative
    }
}

fn generate_run_id() -> String {
    static RUN_ID_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let sequence = RUN_ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("run_{}_{}_{}", timestamp, std::process::id(), sequence)
}

#[derive(Debug)]
struct Story {
    id: String,
    status: String,
    lane: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedConfig;

    fn config() -> ResolvedConfig {
        ResolvedConfig {
            version: 1,
            repo_root: PathBuf::from("/repo"),
            harness_db: PathBuf::from("/repo/harness.db"),
            state_db: PathBuf::from("/repo/.symphony/state.db"),
            runs_dir: PathBuf::from("/repo/.harness/runs"),
            worktrees_dir: PathBuf::from("/repo/.symphony/worktrees"),
            single_active_run: true,
            agent_adapter: "custom".to_owned(),
            agent_command: vec![],
            agent_timeout_minutes: 120,
            pull_request_create: "ask".to_owned(),
            pull_request_provider: "github".to_owned(),
            pull_request_draft_for: vec![],
            changeset_directory: PathBuf::from("/repo/.harness/changesets"),
            changeset_render_in_summary: true,
            allow_here_for_tiny: true,
            compact_keep_last: 50,
            keep_failed_worktrees: true,
            cleanup_after_sync: false,
            auto_source: "harness-db".to_owned(),
            auto_poll_interval_seconds: 30,
            auto_max_attempts: 3,
        }
    }

    fn config_for_root(root: &Path) -> ResolvedConfig {
        let mut config = config();
        config.repo_root = root.to_path_buf();
        config.harness_db = root.join("harness.db");
        config.state_db = root.join(".symphony/state.db");
        config.runs_dir = root.join(".harness/runs");
        config.worktrees_dir = root.join(".symphony/worktrees");
        config.changeset_directory = root.join(".harness/changesets");
        config
    }

    fn write_story_db(path: &Path, id: &str, status: &str, lane: &str) {
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
                "INSERT INTO story (id, title, status, risk_lane) VALUES (?1, ?2, ?3, ?4);",
                params![id, "fixture", status, lane],
            )
            .unwrap();
    }

    fn init_git_repo(path: &Path) {
        for args in [
            vec!["init"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "user.name", "Test User"],
        ] {
            let output = Command::new("git")
                .args(args)
                .current_dir(path)
                .output()
                .unwrap();
            assert!(output.status.success());
        }
        fs::write(path.join("README.md"), "test\n").unwrap();
        assert!(Command::new("git")
            .args(["add", "README.md"])
            .current_dir(path)
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .status()
            .unwrap()
            .success());
    }

    fn replacement_feedback() -> ReplacementFeedback {
        ReplacementFeedback {
            source_run_id: "run_old".to_owned(),
            reason: "Fix mobile spacing".to_owned(),
            evidence: vec![FeedbackFile {
                extension: "png".to_owned(),
                bytes: vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
            }],
        }
    }

    fn add_completed_source(config: &ResolvedConfig) {
        RunStateStore::new(config.state_db.clone())
            .add_run(NewRunRecord {
                run_id: "run_old".to_owned(),
                story_id: "US-084".to_owned(),
                branch: Some("symphony/run_old".to_owned()),
                worktree: config.worktrees_dir.join("run_old"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_old/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "review run result".to_owned(),
            })
            .unwrap();
    }

    #[test]
    fn contract_contains_required_run_fields() {
        let config = config();
        let contract = build_contract(
            &config,
            "run_1",
            "US-036",
            false,
            Path::new("/repo/.symphony/worktrees/run_1"),
            Path::new("/repo/.symphony/worktrees/run_1/harness.db"),
        );

        assert_eq!(contract.version, 1);
        assert_eq!(contract.mode, "execute");
        assert_eq!(contract.story_id, "US-036");
        assert_eq!(contract.worktree, ".symphony/worktrees/run_1");
        assert_eq!(
            contract.harness_db_path,
            ".symphony/worktrees/run_1/harness.db"
        );
        assert_eq!(contract.env.harness_run_id, "run_1");
        assert_eq!(contract.env.harness_run_mode, "execute");
        assert!(contract
            .required_outputs
            .contains(&".harness/runs/run_1/RESULT.json".to_owned()));
        assert_eq!(
            contract.result_json_schema["validation"]["commands"][0]["result"],
            "pass | fail | unavailable"
        );
        assert!(contract.forbidden_paths.contains(&"harness.db".to_owned()));
        assert!(contract.agent_instructions.iter().any(|instruction| {
            instruction.contains("top-level validation object")
                && instruction.contains("not validation_evidence")
        }));
        assert!(!contract.lightweight);
    }

    #[test]
    fn request_changes_contract_contains_feedback_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        init_git_repo(temp_dir.path());
        write_story_db(&config.harness_db, "US-084", "planned", "high-risk");
        add_completed_source(&config);

        let prepared = prepare_replacement_run(&config, "US-084", replacement_feedback()).unwrap();
        let contract: RunContract =
            serde_json::from_str(&fs::read_to_string(&prepared.contract_path).unwrap()).unwrap();
        let feedback = contract.request_changes.unwrap();

        assert_eq!(feedback.source_run_id, "run_old");
        assert!(feedback.reason_path.ends_with("/feedback/reason.md"));
        assert_eq!(feedback.evidence_paths.len(), 1);
        assert!(feedback.evidence_paths[0].ends_with("/feedback/evidence-01.png"));
        assert_eq!(
            fs::read_to_string(
                config
                    .runs_dir
                    .join(&prepared.run_id)
                    .join("feedback/reason.md")
            )
            .unwrap(),
            "Fix mobile spacing\n"
        );
        assert!(prepared
            .worktree
            .join(format!(
                ".harness/runs/{}/feedback/evidence-01.png",
                prepared.run_id
            ))
            .exists());
        assert_eq!(
            RunStateStore::new(config.state_db.clone())
                .show_run("run_old")
                .unwrap()
                .status,
            "rejected"
        );
    }

    #[test]
    fn request_changes_failed_state_commit_removes_prepared_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        init_git_repo(temp_dir.path());
        write_story_db(&config.harness_db, "US-084", "planned", "high-risk");
        add_completed_source(&config);

        let prepared =
            prepare_replacement_files(&config, "US-084", replacement_feedback()).unwrap();
        RunStateStore::new(config.state_db.clone())
            .add_run(NewRunRecord {
                run_id: "run_active".to_owned(),
                story_id: "US-ACTIVE".to_owned(),
                branch: Some("symphony/run_active".to_owned()),
                worktree: config.worktrees_dir.join("run_active"),
                lightweight: false,
                status: "prepared".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_active/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "continue run".to_owned(),
            })
            .unwrap();
        let run_id = prepared.run_id.clone();
        let worktree = prepared.worktree.clone();
        let branch = prepared.branch.clone().unwrap();

        let error = finalize_replacement_run(&config, "run_old", "Fix mobile spacing", prepared)
            .unwrap_err();

        assert!(matches!(
            error,
            RunError::State(StateError::ActiveRunExists(id)) if id == "run_active"
        ));
        assert!(!config.runs_dir.join(&run_id).exists());
        assert!(!worktree.exists());
        let branches = Command::new("git")
            .args(["branch", "--list", &branch])
            .current_dir(&config.repo_root)
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&branches.stdout).trim().is_empty());
        assert_eq!(
            RunStateStore::new(config.state_db.clone())
                .show_run("run_old")
                .unwrap()
                .status,
            "completed"
        );
    }

    #[test]
    fn here_contract_marks_lightweight_and_repo_root() {
        let config = config();
        let contract = build_contract(
            &config,
            "run_1",
            "US-TINY",
            true,
            Path::new("/repo"),
            Path::new("/repo/.symphony/runs/run_1/harness.db"),
        );

        assert!(contract.lightweight);
        assert_eq!(contract.worktree, ".");
        assert_eq!(contract.harness_db_path, ".symphony/runs/run_1/harness.db");
        assert!(contract
            .forbidden_paths
            .contains(&".symphony/runs/**".to_owned()));
    }

    #[test]
    fn agents_shim_names_contract_and_boundaries() {
        let config = config();
        let contract = build_contract(
            &config,
            "run_1",
            "US-037",
            false,
            Path::new("/repo/.symphony/worktrees/run_1"),
            Path::new("/repo/.symphony/worktrees/run_1/harness.db"),
        );
        let shim = render_agents_shim(
            Path::new("/repo/.harness/runs/run_1/RUN_CONTRACT.json"),
            &contract,
        );

        assert!(shim.contains("HARNESS-SYMPHONY:BEGIN"));
        assert!(shim.contains("US-037"));
        assert!(shim.contains("RUN_CONTRACT.json"));
        assert!(shim.contains("HARNESS_DB_PATH=.symphony/worktrees/run_1/harness.db"));
        assert!(shim.contains("Forbidden to commit"));
    }

    #[test]
    fn outcome_and_validation_rules_match_finish_protocol() {
        assert!(valid_outcome("completed"));
        assert!(valid_outcome("blocked"));
        assert!(valid_outcome("needs_intake"));
        assert!(valid_outcome("partial"));
        assert!(valid_outcome("failed"));
        assert!(valid_outcome("cancelled"));
        assert!(!valid_outcome("done"));

        let commands = ResultValidation {
            commands: Some(vec![ValidationCommand {
                command: "cargo test".to_owned(),
                result: "pass".to_owned(),
            }]),
            unavailable: None,
        };
        assert!(has_validation_evidence(Some(&commands)));

        let unavailable = ResultValidation {
            commands: None,
            unavailable: Some("manual validation not available in fixture".to_owned()),
        };
        assert!(has_validation_evidence(Some(&unavailable)));

        let missing = ResultValidation {
            commands: Some(Vec::new()),
            unavailable: None,
        };
        assert!(!has_validation_evidence(Some(&missing)));
    }

    #[test]
    fn parse_result_rejects_invalid_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result_path = temp_dir.path().join("RESULT.json");
        fs::write(&result_path, "{").unwrap();

        assert!(parse_result_file(&result_path).is_err());
    }

    #[test]
    fn prepare_here_run_requires_tiny_lane() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        write_story_db(&config.harness_db, "US-NORMAL", "planned", "normal");

        let error = prepare_here_run(&config, "US-NORMAL").unwrap_err();

        assert!(matches!(
            error,
            RunError::StoryNotTiny { id, lane }
                if id == "US-NORMAL" && lane == "normal"
        ));
        assert!(!config.worktrees_dir.exists());
    }

    #[test]
    fn prepare_run_checks_active_lock_before_creating_worktree_artifacts() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(NewRunRecord {
                run_id: "run_active".to_owned(),
                story_id: "US-ACTIVE".to_owned(),
                branch: Some("symphony/run_active".to_owned()),
                worktree: config.worktrees_dir.join("run_active"),
                lightweight: false,
                status: "prepared".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_active/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "continue active run".to_owned(),
            })
            .unwrap();

        let error = prepare_run(&config, "US-NEXT").unwrap_err();

        assert!(matches!(
            error,
            RunError::State(StateError::ActiveRunExists(id)) if id == "run_active"
        ));
        assert!(!config.worktrees_dir.exists());
        assert!(!config.runs_dir.exists());
    }

    #[test]
    fn prepare_here_run_copies_db_to_run_dir_and_records_lightweight_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        write_story_db(&config.harness_db, "US-TINY", "planned", "tiny");

        let prepared = prepare_here_run(&config, "US-TINY").unwrap();

        assert!(prepared.lightweight);
        assert_eq!(prepared.branch, None);
        assert_eq!(prepared.worktree, config.repo_root);
        assert!(prepared.harness_db_path.exists());
        assert!(prepared
            .harness_db_path
            .starts_with(temp_dir.path().join(".symphony/runs")));
        assert!(!config.worktrees_dir.exists());

        let run = RunStateStore::new(config.state_db.clone())
            .show_run(&prepared.run_id)
            .unwrap();
        assert!(run.lightweight);
        assert_eq!(run.branch, None);
    }

    #[test]
    fn generated_run_ids_are_unique_for_immediate_calls() {
        let first = generate_run_id();
        let second = generate_run_id();

        assert_ne!(first, second);
        assert!(first.starts_with("run_"));
        assert!(second.starts_with("run_"));
    }

    #[test]
    fn lightweight_finished_run_appends_summary_marker() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        let run_id = "run_light";
        let run_dir = temp_dir.path().join(".harness/runs").join(run_id);
        let changeset_dir = temp_dir.path().join(".harness/changesets");
        fs::create_dir_all(&run_dir).unwrap();
        fs::create_dir_all(&changeset_dir).unwrap();
        let summary_path = run_dir.join("SUMMARY.md");
        fs::write(&summary_path, "# Summary\n").unwrap();
        fs::write(
            changeset_dir.join("run_light.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_light"}"#,
        )
        .unwrap();
        fs::write(
            run_dir.join("RESULT.json"),
            r#"{
                "version": 1,
                "run_id": "run_light",
                "story_id": "US-TINY",
                "outcome": "completed",
                "validation": {
                    "commands": [
                        { "command": "cargo test", "result": "pass" }
                    ]
                },
                "summary_path": ".harness/runs/run_light/SUMMARY.md"
            }"#,
        )
        .unwrap();

        let prepared = PreparedRun {
            run_id: run_id.to_owned(),
            story_id: "US-TINY".to_owned(),
            branch: None,
            worktree: temp_dir.path().to_path_buf(),
            contract_path: run_dir.join("RUN_CONTRACT.json"),
            harness_db_path: run_dir.join("harness.db"),
            lightweight: true,
            request_changes: None,
        };

        let completed = validate_finished_run(&config, prepared).unwrap();

        assert_eq!(completed.outcome, "completed");
        let summary = fs::read_to_string(summary_path).unwrap();
        assert!(summary.contains("lightweight: true"));
    }

    #[test]
    fn isolated_finished_run_promotes_review_artifacts() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        let worktree = temp_dir.path().join(".symphony/worktrees/run_full");
        Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        fs::create_dir_all(worktree.join(".harness/runs/run_full")).unwrap();
        fs::create_dir_all(worktree.join(".harness/changesets")).unwrap();
        fs::write(
            worktree.join(".harness/runs/run_full/SUMMARY.md"),
            "# Summary\n",
        )
        .unwrap();
        fs::write(
            worktree.join(".harness/runs/run_full/RESULT.json"),
            r#"{
                "version": 1,
                "run_id": "run_full",
                "story_id": "US-NORMAL",
                "outcome": "completed",
                "validation": {
                    "commands": [
                        { "command": "cargo test", "result": "pass" }
                    ]
                },
                "summary_path": ".harness/runs/run_full/SUMMARY.md"
            }"#,
        )
        .unwrap();
        fs::write(
            worktree.join(".harness/changesets/run_full.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_full"}"#,
        )
        .unwrap();

        let prepared = PreparedRun {
            run_id: "run_full".to_owned(),
            story_id: "US-NORMAL".to_owned(),
            branch: Some("symphony/run_full".to_owned()),
            worktree,
            contract_path: config.runs_dir.join("run_full/RUN_CONTRACT.json"),
            harness_db_path: temp_dir
                .path()
                .join(".symphony/worktrees/run_full/harness.db"),
            lightweight: false,
            request_changes: None,
        };

        let completed = validate_finished_run(&config, prepared).unwrap();

        assert_eq!(
            completed.summary_path,
            config.runs_dir.join("run_full/SUMMARY.md")
        );
        assert_eq!(
            completed.result_path,
            config.runs_dir.join("run_full/RESULT.json")
        );
        assert!(completed.summary_path.exists());
        assert!(completed.result_path.exists());
        assert!(config.runs_dir.join("run_full/changeset.jsonl").exists());
        assert!(!config
            .changeset_directory
            .join("run_full.changeset.jsonl")
            .exists());
    }

    #[test]
    fn isolated_invalid_result_still_promotes_review_artifacts() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config_for_root(temp_dir.path());
        let worktree = temp_dir.path().join(".symphony/worktrees/run_invalid");
        fs::create_dir_all(worktree.join(".harness/runs/run_invalid")).unwrap();
        fs::write(
            worktree.join(".harness/runs/run_invalid/SUMMARY.md"),
            "# Summary\n",
        )
        .unwrap();
        fs::write(
            worktree.join(".harness/runs/run_invalid/RESULT.json"),
            r#"{
                "version": 1,
                "run_id": "run_invalid",
                "story_id": "US-WRONG",
                "outcome": "completed",
                "validation": {
                    "commands": [
                        { "command": "cargo test", "result": "pass" }
                    ]
                },
                "summary_path": ".harness/runs/run_invalid/SUMMARY.md"
            }"#,
        )
        .unwrap();

        let prepared = PreparedRun {
            run_id: "run_invalid".to_owned(),
            story_id: "US-EXPECTED".to_owned(),
            branch: Some("symphony/run_invalid".to_owned()),
            worktree,
            contract_path: config.runs_dir.join("run_invalid/RUN_CONTRACT.json"),
            harness_db_path: temp_dir
                .path()
                .join(".symphony/worktrees/run_invalid/harness.db"),
            lightweight: false,
            request_changes: None,
        };

        let error = validate_finished_run(&config, prepared).unwrap_err();

        assert!(error.to_string().contains("RESULT.json story_id mismatch"));
        assert!(config.runs_dir.join("run_invalid/SUMMARY.md").exists());
        assert!(config.runs_dir.join("run_invalid/RESULT.json").exists());
    }
}
