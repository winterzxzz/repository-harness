use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use crate::config::ResolvedConfig;
use crate::run::result_has_passing_completion_proof;
use crate::state::{RunRecord, RunStateStore, StateError};

#[derive(Debug, Error)]
pub enum PrError {
    #[error("PR creation is disabled by pull_request.create")]
    Disabled,
    #[error("run outcome {0} should not open a PR by default")]
    OutcomeNotAllowed(String),
    #[error("run artifacts are missing for {0}")]
    MissingArtifacts(String),
    #[error("run completion proof is not passing: {0}")]
    InvalidCompletionProof(String),
    #[error("unsupported PR provider: {0}")]
    UnsupportedProvider(String),
    #[error("gh command failed: {0}")]
    GhFailed(String),
    #[error("git command failed: {0}")]
    GitFailed(String),
    #[error("{0}")]
    State(#[from] StateError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrPlan {
    pub run_id: String,
    pub draft: bool,
    pub title: String,
    pub body_path: PathBuf,
    pub files: Vec<PathBuf>,
    pub base_branch: String,
    pub head_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrCreateResult {
    pub plan: PrPlan,
    pub url: Option<String>,
}

pub fn plan_pr(config: &ResolvedConfig, run: &RunRecord) -> Result<PrPlan, PrError> {
    if matches!(config.pull_request_create.as_str(), "disabled" | "never") {
        return Err(PrError::Disabled);
    }
    if matches!(run.status.as_str(), "failed" | "cancelled") {
        return Err(PrError::OutcomeNotAllowed(run.status.clone()));
    }
    let draft = run.status != "completed"
        && config
            .pull_request_draft_for
            .iter()
            .any(|outcome| outcome == &run.status);
    if run.status != "completed" && !draft {
        return Err(PrError::OutcomeNotAllowed(run.status.clone()));
    }

    let summary = config
        .repo_root
        .join(format!(".harness/runs/{}/SUMMARY.md", run.run_id));
    let result = config
        .repo_root
        .join(format!(".harness/runs/{}/RESULT.json", run.run_id));
    let changeset = run.worktree.join(format!(
        ".harness/changesets/{}.changeset.jsonl",
        run.run_id
    ));
    if !summary.exists() || !result.exists() {
        return Err(PrError::MissingArtifacts(run.run_id.clone()));
    }
    ensure_completion_proof(run, &result)?;
    // The changeset only exists when the run wrote durable Harness records;
    // a code/docs-only run is still allowed to open a PR without one.
    let files = if changeset.exists() {
        vec![changeset]
    } else {
        Vec::new()
    };

    let base_branch = default_base_branch(&config.repo_root)?;
    let head_branch = run
        .branch
        .clone()
        .ok_or_else(|| PrError::GitFailed(format!("run {} has no review branch", run.run_id)))?;

    Ok(PrPlan {
        run_id: run.run_id.clone(),
        draft,
        title: format!("{}: {}", run.story_id, run.status),
        body_path: summary.clone(),
        files,
        base_branch,
        head_branch,
    })
}

pub fn create_pr(
    config: &ResolvedConfig,
    run_id: &str,
    dry_run: bool,
) -> Result<PrCreateResult, PrError> {
    let store = RunStateStore::new(config.state_db.clone());
    let run = store.show_run(run_id)?;
    let plan = plan_pr(config, &run)?;
    ensure_forbidden_files_not_staged(&config.repo_root)?;
    if dry_run {
        return Ok(PrCreateResult { plan, url: None });
    }
    if config.pull_request_provider != "github" {
        return Err(PrError::UnsupportedProvider(
            config.pull_request_provider.clone(),
        ));
    }
    prepare_review_branch(&run, &plan)?;
    run_pre_create_gate(config, &run, || Ok(()))?;
    let mut command = Command::new("gh");
    command
        .args(["pr", "create", "--title", &plan.title, "--body-file"])
        .arg(&plan.body_path)
        .args(["--head", &plan.head_branch, "--base", &plan.base_branch]);
    if plan.draft {
        command.arg("--draft");
    }
    let output = command.current_dir(&config.repo_root).output()?;
    if !output.status.success() {
        return Err(PrError::GhFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    store.update_pr_url(run_id, &url)?;
    Ok(PrCreateResult {
        plan,
        url: Some(url),
    })
}

fn run_pre_create_gate<F>(
    config: &ResolvedConfig,
    run: &RunRecord,
    before_validation: F,
) -> Result<(), PrError>
where
    F: FnOnce() -> Result<(), PrError>,
{
    before_validation()?;
    let result = config.runs_dir.join(&run.run_id).join("RESULT.json");
    ensure_completion_proof(run, &result)
}

fn ensure_completion_proof(run: &RunRecord, result: &Path) -> Result<(), PrError> {
    if run.status != "completed" {
        return Ok(());
    }
    let passing = result_has_passing_completion_proof(result, &run.run_id, &run.story_id)
        .map_err(|error| PrError::InvalidCompletionProof(error.to_string()))?;
    if !passing {
        return Err(PrError::InvalidCompletionProof(run.run_id.clone()));
    }
    Ok(())
}

fn ensure_forbidden_files_not_staged(repo_root: &Path) -> Result<(), PrError> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        return Err(PrError::GitFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let staged = String::from_utf8_lossy(&output.stdout);
    for path in staged.lines().map(str::trim) {
        if is_forbidden_commit_path(path) {
            return Err(PrError::GitFailed(format!(
                "forbidden file staged for PR: {path}"
            )));
        }
    }
    Ok(())
}

fn is_forbidden_commit_path(path: &str) -> bool {
    path == "harness.db" || path.starts_with(".symphony/")
}

fn default_base_branch(repo_root: &Path) -> Result<String, PrError> {
    // Prefer the remote default branch so the PR base does not depend on
    // whichever branch the root checkout happens to be on at PR time.
    let output = Command::new("git")
        .args(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .current_dir(repo_root)
        .output()?;
    if output.status.success() {
        let full = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if let Some(branch) = full.strip_prefix("origin/") {
            if !branch.is_empty() {
                return Ok(branch.to_owned());
            }
        }
    }
    current_branch(repo_root)
}

fn current_branch(repo_root: &Path) -> Result<String, PrError> {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        return Err(PrError::GitFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if branch.is_empty() {
        return Err(PrError::GitFailed(
            "cannot create PR from detached HEAD".to_owned(),
        ));
    }
    Ok(branch)
}

fn prepare_review_branch(run: &RunRecord, plan: &PrPlan) -> Result<(), PrError> {
    if !run.worktree.exists() {
        return Err(PrError::GitFailed(format!(
            "run worktree is missing: {}",
            run.worktree.display()
        )));
    }
    strip_agents_shim_file(&run.worktree.join("AGENTS.md"))?;
    git(&run.worktree, &["add", "-A"])?;
    unstage_local_run_files(&run.worktree)?;
    ensure_forbidden_files_not_staged(&run.worktree)?;
    if has_staged_changes(&run.worktree)? {
        git(
            &run.worktree,
            &["commit", "-m", &format!("{}: {}", plan.run_id, plan.title)],
        )?;
    }
    ensure_forbidden_files_not_committed(&run.worktree, &plan.base_branch)?;
    git(&run.worktree, &["push", "-u", "origin", &plan.head_branch])?;
    Ok(())
}

fn unstage_local_run_files(worktree: &Path) -> Result<(), PrError> {
    git_allow_failure(worktree, &["reset", "--", "harness.db", ".symphony"])?;
    Ok(())
}

/// Remove the Symphony run block from AGENTS.md so the shim never reaches the
/// PR while legitimate agent edits to AGENTS.md are preserved.
fn strip_agents_shim_file(path: &Path) -> Result<(), PrError> {
    if !path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(path)?;
    let stripped = strip_symphony_shim(&content);
    if stripped != content {
        std::fs::write(path, stripped)?;
    }
    Ok(())
}

fn strip_symphony_shim(content: &str) -> String {
    const BEGIN: &str = "<!-- HARNESS-SYMPHONY:BEGIN -->";
    const END: &str = "<!-- HARNESS-SYMPHONY:END -->";
    let mut kept: Vec<&str> = Vec::new();
    let mut in_block = false;
    for line in content.lines() {
        if in_block {
            if line.contains(END) {
                in_block = false;
            }
            continue;
        }
        if line.contains(BEGIN) {
            in_block = true;
            // Drop the single blank separator line the shim writer inserted
            // before the block, without joining surrounding text lines.
            if kept.last().is_some_and(|last| last.trim().is_empty()) {
                kept.pop();
            }
            continue;
        }
        kept.push(line);
    }
    if in_block {
        // A mangled block without its END marker: keep the content rather
        // than risk dropping agent-authored text after the marker.
        return content.to_owned();
    }
    let mut result = kept.join("\n");
    if content.ends_with('\n') && !result.is_empty() {
        result.push('\n');
    }
    result
}

/// Refuse to push a review branch whose commits contain local runtime files.
/// The staged-only check cannot see files the agent already committed.
fn ensure_forbidden_files_not_committed(worktree: &Path, base_branch: &str) -> Result<(), PrError> {
    let mut output = None;
    for base in [base_branch.to_owned(), format!("origin/{base_branch}")] {
        let candidate = Command::new("git")
            .args(["diff", "--name-only", &format!("{base}...HEAD")])
            .current_dir(worktree)
            .output()?;
        if candidate.status.success() {
            output = Some(candidate);
            break;
        }
    }
    let Some(output) = output else {
        // Neither ref resolved (e.g. detached setup without the base branch
        // visible from the worktree). The staged-only check already ran, so
        // degrade loudly instead of failing the PR.
        eprintln!(
            "warning: could not diff review branch against {base_branch}; \
skipping the committed-files forbidden-path check"
        );
        return Ok(());
    };
    let committed = String::from_utf8_lossy(&output.stdout);
    for path in committed.lines().map(str::trim) {
        if is_forbidden_commit_path(path) {
            return Err(PrError::GitFailed(format!(
                "forbidden file committed on review branch: {path}"
            )));
        }
    }
    Ok(())
}

fn has_staged_changes(worktree: &Path) -> Result<bool, PrError> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(worktree)
        .output()?;
    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(PrError::GitFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        )),
    }
}

fn git(worktree: &Path, args: &[&str]) -> Result<(), PrError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(worktree)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(PrError::GitFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

fn git_allow_failure(worktree: &Path, args: &[&str]) -> Result<(), PrError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(worktree)
        .output()?;
    if output.status.success() || output.status.code() == Some(128) {
        Ok(())
    } else {
        Err(PrError::GitFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn init_git_repo(root: &Path) {
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(root)
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.invalid"])
            .current_dir(root)
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(root)
            .status()
            .unwrap();
    }

    fn config(root: &Path) -> ResolvedConfig {
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
            pull_request_draft_for: vec!["blocked".to_owned(), "needs_intake".to_owned()],
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

    fn run(status: &str, root: &Path) -> RunRecord {
        RunRecord {
            run_id: "run_1".to_owned(),
            story_id: "US-041".to_owned(),
            branch: Some("symphony/run_1".to_owned()),
            worktree: root.join(".symphony/worktrees/run_1"),
            lightweight: false,
            status: status.to_owned(),
            result_path: Some(PathBuf::from(".harness/runs/run_1/RESULT.json")),
            pr_url: None,
            pr_status: "missing".to_owned(),
            sync_status: "not_applied".to_owned(),
            next_action: "review".to_owned(),
            agent: "codex".to_owned(),
            owner_pid: None,
            agent_pid: None,
            agent_start_identity: None,
            heartbeat_at: None,
            current_stage: "start".to_owned(),
            cancel_requested: false,
            terminal_reason: None,
            execution_mode: "managed".to_owned(),
            harness_db_digest: None,
            reviewed_at: None,
            reviewer_note: None,
        }
    }

    fn passing_result() -> &'static str {
        r#"{
            "version": 1,
            "run_id": "run_1",
            "story_id": "US-041",
            "outcome": "completed",
            "validation": {
                "commands": [
                    { "command": "cargo test", "result": "pass" }
                ]
            }
        }"#
    }

    #[test]
    fn completed_runs_get_normal_pr_plan() {
        let temp_dir = tempfile::tempdir().unwrap();
        init_git_repo(temp_dir.path());
        let config = config(temp_dir.path());
        fs::create_dir_all(temp_dir.path().join(".harness/runs/run_1")).unwrap();
        fs::create_dir_all(
            temp_dir
                .path()
                .join(".symphony/worktrees/run_1/.harness/changesets"),
        )
        .unwrap();
        fs::write(
            temp_dir.path().join(".harness/runs/run_1/SUMMARY.md"),
            "summary",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join(".harness/runs/run_1/RESULT.json"),
            passing_result(),
        )
        .unwrap();
        fs::write(
            temp_dir
                .path()
                .join(".symphony/worktrees/run_1/.harness/changesets/run_1.changeset.jsonl"),
            "{}",
        )
        .unwrap();

        let plan = plan_pr(&config, &run("completed", temp_dir.path())).unwrap();

        assert!(!plan.draft);
        assert_eq!(
            plan.body_path,
            temp_dir.path().join(".harness/runs/run_1/SUMMARY.md")
        );
        assert_eq!(
            plan.files,
            vec![temp_dir
                .path()
                .join(".symphony/worktrees/run_1/.harness/changesets/run_1.changeset.jsonl")]
        );
        assert_eq!(plan.base_branch, "main");
        assert_eq!(plan.head_branch, "symphony/run_1");
    }

    #[test]
    fn configured_blocked_runs_get_draft_plan() {
        let temp_dir = tempfile::tempdir().unwrap();
        init_git_repo(temp_dir.path());
        let config = config(temp_dir.path());
        fs::create_dir_all(temp_dir.path().join(".harness/runs/run_1")).unwrap();
        fs::create_dir_all(
            temp_dir
                .path()
                .join(".symphony/worktrees/run_1/.harness/changesets"),
        )
        .unwrap();
        fs::write(
            temp_dir.path().join(".harness/runs/run_1/SUMMARY.md"),
            "summary",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join(".harness/runs/run_1/RESULT.json"),
            "{}",
        )
        .unwrap();
        fs::write(
            temp_dir
                .path()
                .join(".symphony/worktrees/run_1/.harness/changesets/run_1.changeset.jsonl"),
            "{}",
        )
        .unwrap();

        let plan = plan_pr(&config, &run("blocked", temp_dir.path())).unwrap();

        assert!(plan.draft);
    }

    #[test]
    fn completed_run_rechecks_promoted_result_before_ready_pr() {
        let temp_dir = tempfile::tempdir().unwrap();
        init_git_repo(temp_dir.path());
        let config = config(temp_dir.path());
        fs::create_dir_all(temp_dir.path().join(".harness/runs/run_1")).unwrap();
        fs::write(
            temp_dir.path().join(".harness/runs/run_1/SUMMARY.md"),
            "summary",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join(".harness/runs/run_1/RESULT.json"),
            r#"{
                "version": 1,
                "run_id": "run_1",
                "story_id": "US-041",
                "outcome": "completed",
                "validation": {
                    "commands": [
                        { "command": "cargo test", "result": "fail" }
                    ]
                }
            }"#,
        )
        .unwrap();

        assert!(matches!(
            plan_pr(&config, &run("completed", temp_dir.path())).unwrap_err(),
            PrError::InvalidCompletionProof(_)
        ));
    }

    #[test]
    fn pre_create_gate_rechecks_result_after_mutation() {
        let temp_dir = tempfile::tempdir().unwrap();
        init_git_repo(temp_dir.path());
        let config = config(temp_dir.path());
        fs::create_dir_all(temp_dir.path().join(".harness/runs/run_1")).unwrap();
        fs::write(
            temp_dir.path().join(".harness/runs/run_1/SUMMARY.md"),
            "summary",
        )
        .unwrap();
        let result_path = temp_dir.path().join(".harness/runs/run_1/RESULT.json");
        fs::write(&result_path, passing_result()).unwrap();
        let run = run("completed", temp_dir.path());
        plan_pr(&config, &run).unwrap();

        let error = run_pre_create_gate(&config, &run, || {
            fs::write(
                &result_path,
                r#"{
                    "version": 1,
                    "run_id": "run_1",
                    "story_id": "US-041",
                    "outcome": "completed",
                    "validation": {
                        "commands": [
                            { "command": "cargo test", "result": "fail" }
                        ]
                    }
                }"#,
            )?;
            Ok(())
        })
        .unwrap_err();

        assert!(matches!(error, PrError::InvalidCompletionProof(_)));
    }

    #[test]
    fn plan_allows_missing_changeset_for_docs_only_runs() {
        let temp_dir = tempfile::tempdir().unwrap();
        init_git_repo(temp_dir.path());
        let config = config(temp_dir.path());
        fs::create_dir_all(temp_dir.path().join(".harness/runs/run_1")).unwrap();
        fs::write(
            temp_dir.path().join(".harness/runs/run_1/SUMMARY.md"),
            "summary",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join(".harness/runs/run_1/RESULT.json"),
            passing_result(),
        )
        .unwrap();

        let plan = plan_pr(&config, &run("completed", temp_dir.path())).unwrap();

        assert!(plan.files.is_empty());
    }

    #[test]
    fn strip_symphony_shim_removes_block_and_keeps_agent_edits() {
        let original = "# Agents\n\nkeep this\n";
        let shimmed = format!(
            "{original}\n<!-- HARNESS-SYMPHONY:BEGIN -->\n## Harness Symphony Run\nblock\n<!-- HARNESS-SYMPHONY:END -->\n"
        );
        assert_eq!(strip_symphony_shim(&shimmed), original);

        let edited = "# Agents\n\nkeep this\nagent added a real rule\n\n<!-- HARNESS-SYMPHONY:BEGIN -->\nblock\n<!-- HARNESS-SYMPHONY:END -->\ntrailing agent note\n".to_string();
        let stripped = strip_symphony_shim(&edited);
        assert!(stripped.contains("agent added a real rule"));
        assert!(stripped.contains("trailing agent note"));
        assert!(!stripped.contains("HARNESS-SYMPHONY"));

        assert_eq!(strip_symphony_shim(original), original);

        let double = "# Agents\n\n<!-- HARNESS-SYMPHONY:BEGIN -->\none\n<!-- HARNESS-SYMPHONY:END -->\n\n<!-- HARNESS-SYMPHONY:BEGIN -->\ntwo\n<!-- HARNESS-SYMPHONY:END -->\n".to_string();
        assert!(!strip_symphony_shim(&double).contains("HARNESS-SYMPHONY"));

        let mangled =
            "# Agents\n\n<!-- HARNESS-SYMPHONY:BEGIN -->\nagent text after a mangled block\n";
        assert_eq!(strip_symphony_shim(mangled), mangled);

        let mid_file =
            "line1\n<!-- HARNESS-SYMPHONY:BEGIN -->\nblock\n<!-- HARNESS-SYMPHONY:END -->\nline2\n";
        assert_eq!(strip_symphony_shim(mid_file), "line1\nline2\n");
    }

    #[test]
    fn failed_runs_do_not_get_pr_by_default() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config(temp_dir.path());

        assert!(matches!(
            plan_pr(&config, &run("failed", temp_dir.path())).unwrap_err(),
            PrError::OutcomeNotAllowed(_)
        ));
    }
}
