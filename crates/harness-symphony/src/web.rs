use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::changeset::{render_changeset, render_markdown, ChangesetError};
use crate::config::ResolvedConfig;
use crate::pr::{create_pr, PrError};
use crate::run::{
    execute_prepared_run, prepare_replacement_run, prepare_run, FeedbackFile, PreparedRun,
    ReplacementFeedback, RunContract, RunError,
};
use crate::state::{RunStateStore, StateError};
use crate::sync::{sync_changeset, SyncChange, SyncError};
use crate::upload::{HttpRequest, UploadError};
use crate::work::{
    create_story_from_guided_intake, list_board, retire_story, BoardItem, BoardState, CreatedStory,
    GuidedIntakeDraft, WorkError,
};

const WEB_DIST_DIR_ENV: &str = "HARNESS_SYMPHONY_WEB_DIST_DIR";

#[derive(Debug, Error)]
pub enum WebError {
    #[error("{0}")]
    Work(#[from] WorkError),
    #[error("{0}")]
    Run(#[from] RunError),
    #[error("{0}")]
    State(#[from] StateError),
    #[error("{0}")]
    Changeset(#[from] ChangesetError),
    #[error("{0}")]
    Pr(#[from] PrError),
    #[error("{0}")]
    Sync(#[from] SyncError),
    #[error("{0}")]
    Upload(#[from] UploadError),
    #[error("web server io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("requested web asset is outside the web UI dist directory")]
    InvalidAssetPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebServerOptions {
    pub host: String,
    pub port: u16,
    pub open_browser: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HttpResponse {
    bytes: Vec<u8>,
}

impl HttpResponse {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[cfg(test)]
    fn starts_with(&self, value: &str) -> bool {
        self.bytes.starts_with(value.as_bytes())
    }

    #[cfg(test)]
    fn ends_with(&self, value: &str) -> bool {
        self.bytes.ends_with(value.as_bytes())
    }

    #[cfg(test)]
    fn contains(&self, value: &str) -> bool {
        self.bytes
            .windows(value.len())
            .any(|window| window == value.as_bytes())
    }

    #[cfg(test)]
    fn body(&self) -> &[u8] {
        self.bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| &self.bytes[index + 4..])
            .unwrap_or(&[])
    }
}

#[derive(Debug, Serialize)]
struct BoardResponse {
    items: Vec<BoardItemResponse>,
}

#[derive(Debug, Serialize)]
struct BoardItemResponse {
    id: String,
    title: String,
    board_state: String,
    story_status: String,
    lane: String,
    verify: String,
    blockers: Vec<String>,
    unblocks: Vec<String>,
    parent_id: Option<String>,
    children: Vec<String>,
    hierarchy_depth: usize,
    run_id: Option<String>,
    active_run: Option<String>,
    reason: String,
    failure_summary: Option<FailureSummary>,
    recovery_action: Option<RecoveryAction>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
struct StartRunResponse {
    run_id: String,
    story_id: String,
    status: String,
    agent: String,
}

#[derive(Debug, Deserialize, Default)]
struct StartRunRequest {
    agent: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SettingsPayload {
    default_agent: String,
}

const SELECTABLE_AGENTS: [&str; 2] = ["codex", "opencode"];
const DEFAULT_AGENT_SETTING: &str = "default_agent";

#[derive(Debug, Serialize)]
struct PrRetryResponse {
    run_id: String,
    pr_status: String,
    pr_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct RequestChangesResponse {
    source_run_id: String,
    run_id: String,
    story_id: String,
    status: String,
    feedback: RequestChangesPaths,
}

#[derive(Debug, Serialize)]
struct RequestChangesPaths {
    reason_path: String,
    evidence_paths: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RetireTaskResponse {
    story_id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct GuidedIntakeRequest {
    idea: String,
    audience: String,
    outcome: String,
    non_goals: String,
    validation: String,
}

#[derive(Debug, Serialize)]
struct CreateStoryResponse {
    story_id: String,
    title: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct EventsResponse {
    run_id: String,
    events: Vec<Value>,
}

#[derive(Debug, Serialize)]
struct ReviewResponse {
    run_id: String,
    story_id: String,
    status: String,
    agent: String,
    outcome: Option<String>,
    summary: Option<String>,
    result: Option<Value>,
    validation: Option<Value>,
    changed_files: Vec<String>,
    changeset_preview: Option<String>,
    pr_url: Option<String>,
    pr_status: String,
    artifact_paths: Vec<String>,
    events: Vec<Value>,
    suggested_next_action: String,
    failure_summary: Option<FailureSummary>,
    recovery_action: Option<RecoveryAction>,
    request_changes: Option<ReviewFeedback>,
}

#[derive(Debug, Serialize)]
struct ReviewFeedback {
    reason: String,
    reason_path: String,
    evidence: Vec<ReviewEvidence>,
}

#[derive(Debug, Serialize)]
struct ReviewEvidence {
    path: String,
    url: String,
    content_type: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize)]
struct FailureSummary {
    category: String,
    reason: String,
    latest_event: Option<String>,
    latest_error: Option<String>,
    run_id: String,
    evidence_artifacts: Vec<String>,
    next_action: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct RecoveryAction {
    kind: String,
    label: String,
    endpoint: String,
    confirmation: String,
}

#[derive(Debug, Serialize)]
struct SyncRunResponse {
    run_id: String,
    applied: bool,
    changes: Vec<SyncChangeResponse>,
}

#[derive(Debug, Serialize)]
struct PrMergedResponse {
    run_id: String,
    pr_status: String,
}

#[derive(Debug, Serialize)]
struct SyncChangeResponse {
    id: String,
    path: String,
    applied: bool,
    operations: usize,
}

// New API response structures
#[derive(Debug, Serialize)]
struct ContextResponse {
    story_id: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct TraceResponse {
    traces: Vec<TraceItem>,
    total: usize,
}

#[derive(Debug, Serialize)]
struct TraceItem {
    id: i64,
    story_id: Option<String>,
    summary: String,
    outcome: String,
    created_at: String,
    duration_seconds: Option<i64>,
    harness_friction: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolsResponse {
    tools: Vec<ToolItem>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolItem {
    provider: String,
    name: String,
    kind: String,
    capability: Option<String>,
    status: String,
    description: String,
    responsibility: String,
    command: String,
    source: String,
    since: String,
    scan_target: Option<String>,
    checked_at: Option<String>,
}

fn browser_open_warning(url: &str, error: impl std::fmt::Display) -> String {
    format!("warning: could not open Symphony Web UI at {url}: {error}. Open the URL manually.")
}

fn prepare_web_server<F, E>(
    options: WebServerOptions,
    open_browser: F,
) -> Result<TcpListener, WebError>
where
    F: FnOnce(&str) -> Result<(), E>,
    E: std::fmt::Display,
{
    let listener = TcpListener::bind((options.host.as_str(), options.port))?;
    let address = listener.local_addr()?;
    let browser_ip = if address.ip().is_unspecified() {
        if address.is_ipv6() {
            std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        } else {
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
        }
    } else {
        address.ip()
    };
    let browser_address = std::net::SocketAddr::new(browser_ip, address.port());
    let url = format!("http://{browser_address}");
    println!("Symphony Web UI Controller listening at {url}");
    if !address.ip().is_loopback() {
        eprintln!(
            "warning: the Symphony Web UI has no authentication and can start agent runs; \
binding to {} exposes it beyond this machine. Use a loopback host unless you trust the network.",
            address.ip()
        );
    }
    if options.open_browser {
        if let Err(error) = open_browser(&url) {
            eprintln!("{}", browser_open_warning(&url, error));
        }
    }
    Ok(listener)
}

pub fn run_web_server(config: &ResolvedConfig, options: WebServerOptions) -> Result<(), WebError> {
    let listener = prepare_web_server(options, webbrowser::open)?;
    serve(config, listener)
}

const CONNECTION_IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

fn serve(config: &ResolvedConfig, listener: TcpListener) -> Result<(), WebError> {
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("warning: symphony web connection accept failed: {error}");
                continue;
            }
        };
        // Idle sockets (e.g. browser preconnects) must not hold a handler
        // thread forever, and one slow connection must not block the rest.
        if let Err(error) = stream.set_read_timeout(Some(CONNECTION_IO_TIMEOUT)) {
            eprintln!("warning: symphony web failed to set read timeout: {error}");
        }
        if let Err(error) = stream.set_write_timeout(Some(CONNECTION_IO_TIMEOUT)) {
            eprintln!("warning: symphony web failed to set write timeout: {error}");
        }
        let config = config.clone();
        let spawned = std::thread::Builder::new()
            .spawn(move || handle_connection(&config, &mut stream));
        if let Err(error) = spawned {
            eprintln!("warning: symphony web failed to spawn connection thread: {error}");
        }
    }
    Ok(())
}

fn handle_connection(config: &ResolvedConfig, stream: &mut (impl std::io::Read + Write)) {
    let response = match handle_stream(config, stream) {
        Ok(response) => response,
        Err(error) => {
            eprintln!("warning: symphony web request failed: {error}");
            return;
        }
    };
    if let Err(error) = stream.write_all(response.as_bytes()) {
        eprintln!("warning: symphony web response write failed: {error}");
    }
}

fn handle_stream(
    config: &ResolvedConfig,
    stream: &mut impl std::io::Read,
) -> Result<HttpResponse, WebError> {
    let request = match crate::upload::read_http_request(stream) {
        Ok(request) => request,
        Err(error) => {
            return json_response(
                400,
                &ErrorResponse {
                    error: error.to_string(),
                },
            );
        }
    };
    match handle_http_request(config, &request) {
        Ok(response) => Ok(response),
        Err(error) => json_response(
            500,
            &ErrorResponse {
                error: error.to_string(),
            },
        ),
    }
}

#[cfg(test)]
fn handle_request(config: &ResolvedConfig, request: &str) -> Result<HttpResponse, WebError> {
    let request = crate::upload::parse_http_request(request.as_bytes())?;
    handle_http_request(config, &request)
}

fn handle_http_request(
    config: &ResolvedConfig,
    request: &HttpRequest,
) -> Result<HttpResponse, WebError> {
    let method = request.method.as_str();
    let path = request.path.as_str();
    match (method, path) {
        ("GET", "/health") => json_response(200, &serde_json::json!({"ok": true})),
        ("GET", "/api/board") => {
            let items = list_board(&config.harness_db, &config.state_db)?;
            let items = items
                .into_iter()
                .map(|item| BoardItemResponse::from_item(config, item))
                .collect::<Vec<_>>();
            json_response(200, &BoardResponse { items })
        }
        ("POST", "/api/intake") => create_guided_intake_response(config, request),
        ("GET", "/api/settings") => settings_response(config),
        ("PUT", "/api/settings") => update_settings_response(config, request),
        ("GET", path) if context_path_story_id(path).is_some() => {
            let story_id = context_path_story_id(path).unwrap_or_default();
            context_response(config, &story_id)
        }
        ("GET", path) if traces_path_query(path).is_some() => {
            let query = traces_path_query(path).unwrap_or_default();
            traces_response(config, &query)
        }
        ("GET", "/api/tools") => tools_response(config),
        ("POST", "/api/tools/check") => tools_check_response(config),
        ("POST", path) if start_path_story_id(path).is_some() => {
            let story_id = start_path_story_id(path).unwrap_or_default();
            start_run_response(config, &story_id, request)
        }
        ("POST", path) if recover_path_story_id(path).is_some() => {
            let story_id = recover_path_story_id(path).unwrap_or_default();
            recover_run_response(config, &story_id)
        }
        ("POST", path) if retire_path_story_id(path).is_some() => {
            let story_id = retire_path_story_id(path).unwrap_or_default();
            retire_task_response(config, &story_id)
        }
        ("GET", path) if events_path_run_id(path).is_some() => {
            let run_id = events_path_run_id(path).unwrap_or_default();
            events_response(config, &run_id)
        }
        ("GET", path) if review_path_run_id(path).is_some() => {
            let run_id = review_path_run_id(path).unwrap_or_default();
            review_response(config, &run_id)
        }
        ("POST", path) if sync_path_run_id(path).is_some() => {
            let run_id = sync_path_run_id(path).unwrap_or_default();
            sync_run_response(config, &run_id)
        }
        ("POST", path) if pr_merged_path_run_id(path).is_some() => {
            let run_id = pr_merged_path_run_id(path).unwrap_or_default();
            pr_merged_response(config, &run_id)
        }
        ("POST", path) if pr_retry_path_run_id(path).is_some() => {
            let run_id = pr_retry_path_run_id(path).unwrap_or_default();
            pr_retry_response(config, &run_id)
        }
        ("POST", path) if request_changes_path_run_id(path).is_some() => {
            let run_id = request_changes_path_run_id(path).unwrap_or_default();
            request_changes_response(config, &run_id, request)
        }
        ("GET", path) if feedback_path_parts(path).is_some() => {
            let (run_id, filename) = feedback_path_parts(path).unwrap_or_default();
            feedback_evidence_response(config, &run_id, &filename)
        }
        ("GET", path) => static_response(config, path),
        (_, "/health" | "/api/board" | "/api/intake" | "/api/settings") => json_response(
            405,
            &ErrorResponse {
                error: "method not allowed".to_owned(),
            },
        ),
        (_, path)
            if start_path_story_id(path).is_some()
                || recover_path_story_id(path).is_some()
                || retire_path_story_id(path).is_some()
                || context_path_story_id(path).is_some()
                || traces_path_query(path).is_some()
                || events_path_run_id(path).is_some()
                || review_path_run_id(path).is_some()
                || sync_path_run_id(path).is_some()
                || pr_merged_path_run_id(path).is_some()
                || pr_retry_path_run_id(path).is_some()
                || request_changes_path_run_id(path).is_some()
                || feedback_path_parts(path).is_some()
                || matches!(path, "/api/tools" | "/api/tools/check") =>
        {
            json_response(
                405,
                &ErrorResponse {
                    error: "method not allowed".to_owned(),
                },
            )
        }
        _ => json_response(
            404,
            &ErrorResponse {
                error: "not found".to_owned(),
            },
        ),
    }
}

fn create_guided_intake_response(
    config: &ResolvedConfig,
    request: &HttpRequest,
) -> Result<HttpResponse, WebError> {
    let payload = match serde_json::from_slice::<GuidedIntakeRequest>(&request.body) {
        Ok(payload) => payload,
        Err(error) => {
            return json_response(
                400,
                &ErrorResponse {
                    error: format!("invalid intake request: {error}"),
                },
            );
        }
    };
    match create_story_from_guided_intake(
        &config.harness_db,
        GuidedIntakeDraft {
            idea: payload.idea,
            audience: payload.audience,
            outcome: payload.outcome,
            non_goals: payload.non_goals,
            validation: payload.validation,
        },
    ) {
        Ok(story) => json_response(201, &CreateStoryResponse::from(story)),
        Err(WorkError::InvalidInput(error)) => json_response(400, &ErrorResponse { error }),
        Err(error) => Err(error.into()),
    }
}

fn retire_task_response(config: &ResolvedConfig, story_id: &str) -> Result<HttpResponse, WebError> {
    let item = match list_board(&config.harness_db, &config.state_db)?
        .into_iter()
        .find(|item| item.id == story_id)
    {
        Some(item) => item,
        None => {
            return json_response(
                404,
                &ErrorResponse {
                    error: "story not found".to_owned(),
                },
            );
        }
    };
    if item.board_state != BoardState::Ready {
        return json_response(
            409,
            &ErrorResponse {
                error: "only Ready stories can be retired".to_owned(),
            },
        );
    }
    retire_story(&config.harness_db, story_id)?;
    json_response(
        200,
        &RetireTaskResponse {
            story_id: story_id.to_owned(),
            status: "retired".to_owned(),
        },
    )
}

impl From<CreatedStory> for CreateStoryResponse {
    fn from(story: CreatedStory) -> Self {
        Self {
            story_id: story.story_id,
            title: story.title,
            status: story.status,
        }
    }
}

fn recover_run_response(config: &ResolvedConfig, story_id: &str) -> Result<HttpResponse, WebError> {
    let item = match list_board(&config.harness_db, &config.state_db)?
        .into_iter()
        .find(|item| item.id == story_id)
    {
        Some(item) => item,
        None => {
            return json_response(
                404,
                &ErrorResponse {
                    error: "story not found".to_owned(),
                },
            );
        }
    };
    let run_id = match item.run_id.as_deref() {
        Some(run_id) => run_id,
        None => {
            return json_response(
                409,
                &ErrorResponse {
                    error: "latest story state is not recoverable".to_owned(),
                },
            );
        }
    };
    let store = RunStateStore::new(config.state_db.clone());
    let run = store.show_run(run_id)?;
    if recovery_action_for_run(&item.id, &item.story_status, &item.board_state, &run)
        .as_ref()
        .is_none_or(|action| action.kind != "execution_retry")
    {
        return json_response(
            409,
            &ErrorResponse {
                error: "latest story state is not recoverable by execution retry".to_owned(),
            },
        );
    }
    if let Some(active) = store.active_run()? {
        return json_response(
            409,
            &ErrorResponse {
                error: format!("active run already exists: {}", active.run_id),
            },
        );
    }
    let agent = default_agent(config)?;
    let config = config_for_agent(config, &agent);
    match prepare_run(&config, story_id) {
        Ok(prepared) => {
            let response = StartRunResponse {
                run_id: prepared.run_id.clone(),
                story_id: prepared.story_id.clone(),
                status: "recovering".to_owned(),
                agent,
            };
            spawn_run(config.clone(), prepared);
            json_response(202, &response)
        }
        Err(RunError::State(StateError::ActiveRunExists(run_id))) => json_response(
            409,
            &ErrorResponse {
                error: format!("active run already exists: {run_id}"),
            },
        ),
        Err(error) => json_response(
            400,
            &ErrorResponse {
                error: error.to_string(),
            },
        ),
    }
}

fn sync_run_response(config: &ResolvedConfig, run_id: &str) -> Result<HttpResponse, WebError> {
    if !safe_identifier(run_id) {
        return json_response(
            400,
            &ErrorResponse {
                error: "invalid run id".to_owned(),
            },
        );
    }
    let run = match RunStateStore::new(config.state_db.clone()).show_run(run_id) {
        Ok(run) => run,
        Err(StateError::RunNotFound(_)) => {
            return json_response(
                404,
                &ErrorResponse {
                    error: "run not found".to_owned(),
                },
            );
        }
        Err(error) => return Err(error.into()),
    };
    if run.pr_status != "merged" && !local_review_without_pr(config, &run) {
        return json_response(
            409,
            &ErrorResponse {
                error: "pull request must be marked merged before sync".to_owned(),
            },
        );
    }
    let result = sync_changeset(config, run_id)?;
    let changes = result
        .changes
        .into_iter()
        .map(SyncChangeResponse::from)
        .collect::<Vec<_>>();
    let applied = changes
        .iter()
        .any(|change| change.id == run_id && change.applied);
    json_response(
        200,
        &SyncRunResponse {
            run_id: run_id.to_owned(),
            applied,
            changes,
        },
    )
}

fn pr_merged_response(config: &ResolvedConfig, run_id: &str) -> Result<HttpResponse, WebError> {
    if !safe_identifier(run_id) {
        return json_response(
            400,
            &ErrorResponse {
                error: "invalid run id".to_owned(),
            },
        );
    }
    let store = RunStateStore::new(config.state_db.clone());
    match store.show_run(run_id) {
        Ok(run) if run.pr_url.is_some() => {
            store.update_pr_status(run_id, "merged")?;
            json_response(
                200,
                &PrMergedResponse {
                    run_id: run_id.to_owned(),
                    pr_status: "merged".to_owned(),
                },
            )
        }
        Ok(_) => json_response(
            409,
            &ErrorResponse {
                error: "pull request has not been created".to_owned(),
            },
        ),
        Err(StateError::RunNotFound(_)) => json_response(
            404,
            &ErrorResponse {
                error: "run not found".to_owned(),
            },
        ),
        Err(error) => Err(error.into()),
    }
}

fn pr_retry_response(config: &ResolvedConfig, run_id: &str) -> Result<HttpResponse, WebError> {
    if !safe_identifier(run_id) {
        return json_response(
            400,
            &ErrorResponse {
                error: "invalid run id".to_owned(),
            },
        );
    }
    let store = RunStateStore::new(config.state_db.clone());
    let run = match store.show_run(run_id) {
        Ok(run) => run,
        Err(StateError::RunNotFound(_)) => {
            return json_response(
                404,
                &ErrorResponse {
                    error: "run not found".to_owned(),
                },
            );
        }
        Err(error) => return Err(error.into()),
    };
    if let Some(active) = store.active_run()? {
        return json_response(
            409,
            &ErrorResponse {
                error: format!("active run already exists: {}", active.run_id),
            },
        );
    }
    let action = recovery_action_for_review(config, &run)?;
    if action
        .as_ref()
        .is_none_or(|action| action.kind != "pr_retry")
    {
        return json_response(
            409,
            &ErrorResponse {
                error: "run is not recoverable by PR retry".to_owned(),
            },
        );
    }
    if let Err(error) = create_review_pr(config, run_id) {
        return json_response(
            409,
            &ErrorResponse {
                error: error.to_string(),
            },
        );
    }
    let updated = store.show_run(run_id)?;
    json_response(
        200,
        &PrRetryResponse {
            run_id: run_id.to_owned(),
            pr_status: updated.pr_status,
            pr_url: updated.pr_url,
        },
    )
}

fn default_agent(config: &ResolvedConfig) -> Result<String, WebError> {
    let stored = RunStateStore::new(config.state_db.clone()).get_setting(DEFAULT_AGENT_SETTING)?;
    Ok(stored.unwrap_or_else(|| config.agent_adapter.clone()))
}

fn settings_response(config: &ResolvedConfig) -> Result<HttpResponse, WebError> {
    json_response(
        200,
        &SettingsPayload {
            default_agent: default_agent(config)?,
        },
    )
}

fn update_settings_response(
    config: &ResolvedConfig,
    request: &HttpRequest,
) -> Result<HttpResponse, WebError> {
    let payload = match serde_json::from_slice::<SettingsPayload>(&request.body) {
        Ok(payload) => payload,
        Err(error) => {
            return json_response(
                400,
                &ErrorResponse {
                    error: format!("invalid settings request: {error}"),
                },
            );
        }
    };
    if !SELECTABLE_AGENTS.contains(&payload.default_agent.as_str()) {
        return json_response(
            400,
            &ErrorResponse {
                error: format!(
                    "unsupported agent '{}'. Selectable agents: {}",
                    payload.default_agent,
                    SELECTABLE_AGENTS.join(", ")
                ),
            },
        );
    }
    RunStateStore::new(config.state_db.clone())
        .set_setting(DEFAULT_AGENT_SETTING, &payload.default_agent)?;
    json_response(200, &payload)
}

fn config_for_agent(config: &ResolvedConfig, agent: &str) -> ResolvedConfig {
    let mut effective = config.clone();
    if effective.agent_adapter != agent {
        effective.agent_adapter = agent.to_owned();
        effective.agent_command = Vec::new();
    }
    effective
}

fn start_run_response(
    config: &ResolvedConfig,
    story_id: &str,
    request: &HttpRequest,
) -> Result<HttpResponse, WebError> {
    let payload = if request.body.iter().all(u8::is_ascii_whitespace) {
        StartRunRequest::default()
    } else {
        match serde_json::from_slice::<StartRunRequest>(&request.body) {
            Ok(payload) => payload,
            Err(error) => {
                return json_response(
                    400,
                    &ErrorResponse {
                        error: format!("invalid start request: {error}"),
                    },
                );
            }
        }
    };
    let requested_agent = payload.agent;
    if let Some(agent) = requested_agent.as_deref() {
        if !SELECTABLE_AGENTS.contains(&agent) {
            return json_response(
                400,
                &ErrorResponse {
                    error: format!(
                        "unsupported agent '{agent}'. Selectable agents: {}",
                        SELECTABLE_AGENTS.join(", ")
                    ),
                },
            );
        }
    }
    let agent = match requested_agent.clone() {
        Some(agent) => agent,
        None => default_agent(config)?,
    };
    let config = config_for_agent(config, &agent);
    if let Some(active) = RunStateStore::new(config.state_db.clone()).active_run()? {
        return json_response(
            409,
            &ErrorResponse {
                error: format!("active run already exists: {}", active.run_id),
            },
        );
    }
    match prepare_run(&config, story_id) {
        Ok(prepared) => {
            if requested_agent.is_some() {
                RunStateStore::new(config.state_db.clone())
                    .set_setting(DEFAULT_AGENT_SETTING, &agent)?;
            }
            let response = StartRunResponse {
                run_id: prepared.run_id.clone(),
                story_id: prepared.story_id.clone(),
                status: "started".to_owned(),
                agent,
            };
            spawn_run(config.clone(), prepared);
            json_response(202, &response)
        }
        Err(RunError::State(StateError::ActiveRunExists(run_id))) => json_response(
            409,
            &ErrorResponse {
                error: format!("active run already exists: {run_id}"),
            },
        ),
        Err(error) => json_response(
            400,
            &ErrorResponse {
                error: error.to_string(),
            },
        ),
    }
}

fn spawn_run(config: ResolvedConfig, prepared: PreparedRun) {
    std::thread::spawn(move || {
        let run_id = prepared.run_id.clone();
        match execute_prepared_run(&config, prepared) {
            Ok(completed) if completed.outcome == "completed" => {
                if let Err(error) = create_review_pr(&config, &run_id) {
                    eprintln!("web run {run_id} PR creation failed: {error}");
                }
            }
            Ok(_) => {}
            Err(error) => eprintln!("web run {run_id} failed: {error}"),
        }
    });
}

fn create_review_pr(config: &ResolvedConfig, run_id: &str) -> Result<(), WebError> {
    if pr_creation_disabled(config) {
        RunStateStore::new(config.state_db.clone()).update_pr_status(run_id, "not_applicable")?;
        return Ok(());
    }
    if let Err(error) = create_pr(config, run_id, false) {
        RunStateStore::new(config.state_db.clone())
            .record_pr_failure(run_id, &error.to_string())?;
        return Err(error.into());
    }
    Ok(())
}

fn events_response(config: &ResolvedConfig, run_id: &str) -> Result<HttpResponse, WebError> {
    if !safe_identifier(run_id) {
        return json_response(
            400,
            &ErrorResponse {
                error: "invalid run id".to_owned(),
            },
        );
    }
    let event_path = config.runs_dir.join(run_id).join("APP_SERVER_EVENTS.jsonl");
    let events = read_events(&event_path)?;
    json_response(
        200,
        &EventsResponse {
            run_id: run_id.to_owned(),
            events,
        },
    )
}

fn request_changes_response(
    config: &ResolvedConfig,
    run_id: &str,
    request: &HttpRequest,
) -> Result<HttpResponse, WebError> {
    request_changes_response_with_spawn(config, run_id, request, spawn_run)
}

fn request_changes_response_with_spawn<F>(
    config: &ResolvedConfig,
    run_id: &str,
    request: &HttpRequest,
    spawn: F,
) -> Result<HttpResponse, WebError>
where
    F: FnOnce(ResolvedConfig, PreparedRun),
{
    let submission = match crate::upload::parse_request_changes(request) {
        Ok(submission) => submission,
        Err(error) => {
            return json_response(
                400,
                &ErrorResponse {
                    error: error.to_string(),
                },
            );
        }
    };
    let store = RunStateStore::new(config.state_db.clone());
    let source = match store.show_run(run_id) {
        Ok(run) => run,
        Err(StateError::RunNotFound(_)) => {
            return json_response(
                404,
                &ErrorResponse {
                    error: "run not found".to_owned(),
                },
            );
        }
        Err(error) => return Err(error.into()),
    };
    let item = match list_board(&config.harness_db, &config.state_db)?
        .into_iter()
        .find(|item| item.id == source.story_id)
    {
        Some(item) => item,
        None => {
            return json_response(
                409,
                &ErrorResponse {
                    error: "source story is not available on the board".to_owned(),
                },
            );
        }
    };
    if item.run_id.as_deref() != Some(run_id) {
        return json_response(
            409,
            &ErrorResponse {
                error: "run is stale; request changes from the latest Ready result".to_owned(),
            },
        );
    }
    let is_review = item.board_state == BoardState::Review
        || (item.board_state == BoardState::NeedsAttention
            && local_review_without_pr(config, &source));
    if !is_review
        || source.status != "completed"
        || is_synced(&source)
        || !matches!(item.story_status.as_str(), "planned" | "in_progress")
    {
        return json_response(
            409,
            &ErrorResponse {
                error: "request changes is available only for the latest unsynced Ready result"
                    .to_owned(),
            },
        );
    }
    if let Some(active) = store.active_run()? {
        return json_response(
            409,
            &ErrorResponse {
                error: format!("active run already exists: {}", active.run_id),
            },
        );
    }

    let agent = default_agent(config)?;
    let effective_config = config_for_agent(config, &agent);
    let prepared = match prepare_replacement_run(
        &effective_config,
        &source.story_id,
        ReplacementFeedback {
            source_run_id: source.run_id.clone(),
            reason: submission.reason,
            evidence: submission
                .evidence
                .into_iter()
                .map(|file| FeedbackFile {
                    extension: file.extension,
                    bytes: file.bytes,
                })
                .collect(),
        },
    ) {
        Ok(prepared) => prepared,
        Err(RunError::State(StateError::ActiveRunExists(run_id))) => {
            return json_response(
                409,
                &ErrorResponse {
                    error: format!("active run already exists: {run_id}"),
                },
            );
        }
        Err(
            error @ (RunError::State(StateError::RunNotReplaceable { .. })
            | RunError::State(StateError::ReplacementStoryMismatch { .. })
            | RunError::StoryNotRunnable { .. }),
        ) => {
            return json_response(
                409,
                &ErrorResponse {
                    error: error.to_string(),
                },
            );
        }
        Err(error @ RunError::InvalidFeedback(_)) => {
            return json_response(
                400,
                &ErrorResponse {
                    error: error.to_string(),
                },
            );
        }
        Err(error) => {
            return json_response(
                400,
                &ErrorResponse {
                    error: error.to_string(),
                },
            );
        }
    };
    let Some(feedback) = prepared.request_changes.as_ref() else {
        return json_response(
            500,
            &ErrorResponse {
                error: "replacement run is missing request changes metadata".to_owned(),
            },
        );
    };
    let response = RequestChangesResponse {
        source_run_id: source.run_id,
        run_id: prepared.run_id.clone(),
        story_id: prepared.story_id.clone(),
        status: "prepared".to_owned(),
        feedback: RequestChangesPaths {
            reason_path: feedback.reason_path.clone(),
            evidence_paths: feedback.evidence_paths.clone(),
        },
    };
    spawn(effective_config, prepared);
    json_response(202, &response)
}

fn feedback_evidence_response(
    config: &ResolvedConfig,
    run_id: &str,
    filename: &str,
) -> Result<HttpResponse, WebError> {
    if !safe_identifier(run_id) || generated_evidence_content_type(filename).is_none() {
        return json_response(
            400,
            &ErrorResponse {
                error: "invalid feedback evidence path".to_owned(),
            },
        );
    }
    let feedback = match review_feedback(config, run_id)? {
        Some(feedback) => feedback,
        None => {
            return json_response(
                404,
                &ErrorResponse {
                    error: "feedback evidence not found".to_owned(),
                },
            );
        }
    };
    let evidence = match feedback
        .evidence
        .iter()
        .find(|evidence| evidence.url.ends_with(&format!("/{filename}")))
    {
        Some(evidence) => evidence,
        None => {
            return json_response(
                404,
                &ErrorResponse {
                    error: "feedback evidence not found".to_owned(),
                },
            );
        }
    };
    let path = config.runs_dir.join(run_id).join("feedback").join(filename);
    let bytes = fs::read(path)?;
    binary_response(200, &evidence.content_type, bytes)
}

fn review_response(config: &ResolvedConfig, run_id: &str) -> Result<HttpResponse, WebError> {
    if !safe_identifier(run_id) {
        return json_response(
            400,
            &ErrorResponse {
                error: "invalid run id".to_owned(),
            },
        );
    }

    let run = match RunStateStore::new(config.state_db.clone()).show_run(run_id) {
        Ok(run) => run,
        Err(StateError::RunNotFound(_)) => {
            return json_response(
                404,
                &ErrorResponse {
                    error: "run not found".to_owned(),
                },
            );
        }
        Err(error) => return Err(error.into()),
    };

    let run_dir = config.runs_dir.join(run_id);
    let summary_path = run_dir.join("SUMMARY.md");
    let result_path = run_dir.join("RESULT.json");
    let review_changeset_path = run_dir.join("changeset.jsonl");
    let committed_changeset_path = config
        .changeset_directory
        .join(format!("{run_id}.changeset.jsonl"));
    let changeset_path = if review_changeset_path.exists() {
        review_changeset_path
    } else {
        committed_changeset_path
    };
    let event_path = run_dir.join("APP_SERVER_EVENTS.jsonl");

    let summary = read_optional_text(&summary_path)?;
    let result_artifact = read_optional_json_artifact(&result_path);
    let result = result_artifact.value;
    let validation = result
        .as_ref()
        .and_then(|value| value.get("validation").cloned());
    let outcome = result
        .as_ref()
        .and_then(|value| value.get("outcome"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let changed_files = result
        .as_ref()
        .and_then(|value| value.get("changed_files"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let changeset_preview = if changeset_path.exists() {
        let rendered = render_changeset(&changeset_path)?;
        Some(render_markdown(
            &rendered,
            &changeset_display_path(&config.repo_root, &changeset_path),
        ))
    } else {
        None
    };
    let artifact_paths = [&summary_path, &result_path, &changeset_path, &event_path]
        .into_iter()
        .filter(|path| path.exists())
        .map(|path| changeset_display_path(&config.repo_root, path))
        .collect::<Vec<_>>();
    let local_review = local_review_without_pr(config, &run);
    let pr_status = if local_review {
        "not_applicable".to_owned()
    } else {
        run.pr_status.clone()
    };
    let suggested_next_action =
        review_next_action(config, &run, summary.is_some(), result.is_some());
    let events = read_events(&event_path)?;
    let failure_summary = failure_summary_for_run(config, &run);
    let recovery_action = if local_review {
        None
    } else {
        recovery_action_for_review(config, &run)?
    };
    let request_changes = review_feedback(config, run_id)?;

    json_response(
        200,
        &ReviewResponse {
            run_id: run.run_id,
            story_id: run.story_id,
            status: run.status,
            agent: run.agent,
            outcome,
            summary,
            result,
            validation,
            changed_files,
            changeset_preview,
            pr_url: run.pr_url,
            pr_status,
            artifact_paths,
            events,
            suggested_next_action,
            failure_summary,
            recovery_action,
            request_changes,
        },
    )
}

fn review_feedback(
    config: &ResolvedConfig,
    run_id: &str,
) -> Result<Option<ReviewFeedback>, WebError> {
    let run_dir = config.runs_dir.join(run_id);
    let contract_path = run_dir.join("RUN_CONTRACT.json");
    if !contract_path.exists() {
        return Ok(None);
    }
    let contract: RunContract = serde_json::from_slice(&fs::read(contract_path)?)?;
    let Some(feedback) = contract.request_changes else {
        return Ok(None);
    };
    let expected_reason_path = format!(".harness/runs/{run_id}/feedback/reason.md");
    if contract.run_id != run_id || feedback.reason_path != expected_reason_path {
        return Ok(None);
    }
    let feedback_dir = run_dir.join("feedback");
    let Ok(reason) = fs::read_to_string(feedback_dir.join("reason.md")) else {
        return Ok(None);
    };
    let reason = reason.trim().to_owned();
    let expected_prefix = format!(".harness/runs/{run_id}/feedback/");
    let mut evidence = Vec::new();
    for path in feedback.evidence_paths {
        let Some(filename) = path.strip_prefix(&expected_prefix).map(str::to_owned) else {
            return Ok(None);
        };
        let Some(content_type) = generated_evidence_content_type(&filename) else {
            return Ok(None);
        };
        let file_path = feedback_dir.join(&filename);
        if !file_path.is_file() {
            continue;
        }
        let metadata = fs::metadata(&file_path)?;
        if metadata.len() > crate::upload::MAX_EVIDENCE_BYTES as u64 {
            continue;
        }
        evidence.push(ReviewEvidence {
            path,
            url: format!("/api/runs/{run_id}/feedback/{filename}"),
            content_type: content_type.to_owned(),
            size: metadata.len(),
        });
    }
    Ok(Some(ReviewFeedback {
        reason,
        reason_path: feedback.reason_path,
        evidence,
    }))
}

fn changeset_display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn read_optional_text(path: &Path) -> Result<Option<String>, WebError> {
    if path.exists() {
        Ok(Some(fs::read_to_string(path)?))
    } else {
        Ok(None)
    }
}

struct JsonArtifact {
    value: Option<Value>,
    diagnostic: Option<String>,
}

fn read_optional_json_artifact(path: &Path) -> JsonArtifact {
    if !path.exists() {
        return JsonArtifact {
            value: None,
            diagnostic: Some(format!("{} is missing", artifact_name(path))),
        };
    }
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(value) => JsonArtifact {
                value: Some(value),
                diagnostic: None,
            },
            Err(error) => JsonArtifact {
                value: None,
                diagnostic: Some(format!("{} is malformed: {error}", artifact_name(path))),
            },
        },
        Err(error) => JsonArtifact {
            value: None,
            diagnostic: Some(format!(
                "{} could not be read: {error}",
                artifact_name(path)
            )),
        },
    }
}

fn read_events(path: &Path) -> Result<Vec<Value>, WebError> {
    Ok(read_events_artifact(path).events)
}

struct EventsArtifact {
    events: Vec<Value>,
    diagnostic: Option<String>,
}

fn read_events_artifact(path: &Path) -> EventsArtifact {
    if !path.exists() {
        return EventsArtifact {
            events: Vec::new(),
            diagnostic: Some(format!("{} is missing", artifact_name(path))),
        };
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            return EventsArtifact {
                events: Vec::new(),
                diagnostic: Some(format!(
                    "{} could not be read: {error}",
                    artifact_name(path)
                )),
            };
        }
    };
    let mut malformed = 0;
    let events = text
        .lines()
        .filter_map(|line| match serde_json::from_str::<Value>(line) {
            Ok(value) => Some(value),
            Err(_) => {
                malformed += 1;
                None
            }
        })
        .collect::<Vec<_>>();
    let diagnostic = (malformed > 0).then(|| {
        format!(
            "{} has {malformed} malformed JSON line(s); parsed events are still shown",
            artifact_name(path)
        )
    });
    EventsArtifact { events, diagnostic }
}

struct RunArtifactPaths {
    summary: PathBuf,
    result: PathBuf,
    changeset: PathBuf,
    events: PathBuf,
}

fn run_artifact_paths(config: &ResolvedConfig, run_id: &str) -> RunArtifactPaths {
    let run_dir = config.runs_dir.join(run_id);
    let review_changeset_path = run_dir.join("changeset.jsonl");
    let committed_changeset_path = config
        .changeset_directory
        .join(format!("{run_id}.changeset.jsonl"));
    let changeset = if review_changeset_path.exists() {
        review_changeset_path
    } else {
        committed_changeset_path
    };
    RunArtifactPaths {
        summary: run_dir.join("SUMMARY.md"),
        result: run_dir.join("RESULT.json"),
        changeset,
        events: run_dir.join("APP_SERVER_EVENTS.jsonl"),
    }
}

fn failure_summary_for_run(
    config: &ResolvedConfig,
    run: &crate::state::RunRecord,
) -> Option<FailureSummary> {
    if !run_needs_attention(config, run) {
        return None;
    }
    let paths = run_artifact_paths(config, &run.run_id);
    let result_artifact = read_optional_json_artifact(&paths.result);
    let events_artifact = read_events_artifact(&paths.events);
    let result = result_artifact.value.as_ref();
    let latest_event = latest_event_message(&events_artifact.events);
    let event_error = latest_event_error(&events_artifact.events);
    let result_contract_mismatch = result_contract_mismatch(result, run);
    let validation_failure = result.and_then(validation_failure_message);
    let evidence_artifacts = available_artifacts(config, &paths);

    let (category, reason, latest_error, next_action) = if run.pr_status == "failed"
        || run.next_action.starts_with("pull request creation failed")
    {
        (
            "PR creation failure".to_owned(),
            compact_sentence(&run.next_action),
            Some(run.next_action.clone()),
            "Retry pull request creation after fixing the reported PR error.".to_owned(),
        )
    } else if let Some(message) = result_contract_mismatch {
        (
            "Invalid result artifact".to_owned(),
            compact_sentence(&message),
            Some(message),
            "Inspect RESULT.json and rerun the task after the result contract mismatch is understood."
                .to_owned(),
        )
    } else if let Some(message) = validation_failure {
        (
            "Validation failure".to_owned(),
            compact_sentence(&message),
            Some(message),
            "Fix validation failure, rerun proof, then retry or handle manually.".to_owned(),
        )
    } else if let Some(message) = event_error {
        let lower = message.to_lowercase();
        if lower.contains("timeout") || lower.contains("timed out") {
            (
                "Codex app-server timeout".to_owned(),
                compact_sentence(&message),
                Some(message),
                "Inspect APP_SERVER_EVENTS.jsonl and retry when safe; older timeout runs may need manual handling.".to_owned(),
            )
        } else {
            (
                "Codex run failure".to_owned(),
                compact_sentence(&message),
                Some(message),
                "Inspect APP_SERVER_EVENTS.jsonl and retry when safe.".to_owned(),
            )
        }
    } else if !paths.result.exists() {
        (
            "Missing artifact".to_owned(),
            "RESULT.json is missing; the run cannot be reviewed yet.".to_owned(),
            result_artifact.diagnostic.clone(),
            "Inspect run artifacts and rerun the task when the missing artifact is understood."
                .to_owned(),
        )
    } else if let Some(message) = result_artifact.diagnostic.clone() {
        (
            "Malformed artifact".to_owned(),
            compact_sentence(&message),
            Some(message),
            "Inspect run artifacts and rerun the task when the malformed artifact is understood."
                .to_owned(),
        )
    } else if !paths.summary.exists() {
        (
            "Missing artifact".to_owned(),
            "SUMMARY.md is missing; the run needs manual review.".to_owned(),
            Some(format!("{} is missing", artifact_name(&paths.summary))),
            "Inspect run artifacts and rerun the task when the missing artifact is understood."
                .to_owned(),
        )
    } else if let Some(message) = events_artifact.diagnostic.clone() {
        (
            if paths.events.exists() {
                "Malformed event log".to_owned()
            } else {
                "Missing artifact".to_owned()
            },
            compact_sentence(&message),
            Some(message),
            "Inspect APP_SERVER_EVENTS.jsonl and handle missing or malformed event output manually."
                .to_owned(),
        )
    } else {
        (
            "Manual follow-up".to_owned(),
            compact_sentence(&run.next_action),
            None,
            if run.next_action.trim().is_empty() {
                "Handle this run manually.".to_owned()
            } else {
                run.next_action.clone()
            },
        )
    };

    Some(FailureSummary {
        category,
        reason,
        latest_event,
        latest_error,
        run_id: run.run_id.clone(),
        evidence_artifacts,
        next_action,
    })
}

fn result_contract_mismatch(
    result: Option<&Value>,
    run: &crate::state::RunRecord,
) -> Option<String> {
    let result = result?;
    if let Some(actual) = result.get("run_id").and_then(Value::as_str) {
        if actual != run.run_id {
            return Some(format!(
                "RESULT.json run_id mismatch: expected {}, got {}",
                run.run_id, actual
            ));
        }
    }
    if let Some(actual) = result.get("story_id").and_then(Value::as_str) {
        if actual != run.story_id {
            return Some(format!(
                "RESULT.json story_id mismatch: expected {}, got {}",
                run.story_id, actual
            ));
        }
    }
    None
}

fn run_needs_attention(config: &ResolvedConfig, run: &crate::state::RunRecord) -> bool {
    if local_review_without_pr(config, run) {
        return false;
    }
    matches!(
        run.status.as_str(),
        "failed" | "cancelled" | "partial" | "blocked" | "needs_intake"
    ) || (run.status == "completed" && (run.pr_status == "failed" || run.pr_url.is_none()))
}

fn recovery_action_for_review(
    config: &ResolvedConfig,
    run: &crate::state::RunRecord,
) -> Result<Option<RecoveryAction>, WebError> {
    let items = match list_board(&config.harness_db, &config.state_db) {
        Ok(items) => items,
        Err(WorkError::MissingDatabase(_)) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let item = items.into_iter().find(|item| item.id == run.story_id);
    let Some(item) = item else {
        return Ok(None);
    };
    if item.run_id.as_deref() != Some(run.run_id.as_str()) {
        return Ok(None);
    }
    Ok(recovery_action_for_run(
        &item.id,
        &item.story_status,
        &item.board_state,
        run,
    ))
}

fn recovery_action_for_run(
    story_id: &str,
    story_status: &str,
    board_state: &BoardState,
    run: &crate::state::RunRecord,
) -> Option<RecoveryAction> {
    if *board_state != BoardState::NeedsAttention {
        return None;
    }
    if pr_retryable_run(run) {
        return Some(RecoveryAction {
            kind: "pr_retry".to_owned(),
            label: "Retry PR creation".to_owned(),
            endpoint: format!("/api/runs/{}/pr-retry", run.run_id),
            confirmation: "Retry pull request creation for this completed run?".to_owned(),
        });
    }
    if execution_retryable_run(run) && matches!(story_status, "planned" | "in_progress") {
        return Some(RecoveryAction {
            kind: "execution_retry".to_owned(),
            label: "Retry work".to_owned(),
            endpoint: format!("/api/tasks/{story_id}/recover"),
            confirmation:
                "Start a new Symphony run for this task? The failed run evidence stays available."
                    .to_owned(),
        });
    }
    None
}

fn execution_retryable_run(run: &crate::state::RunRecord) -> bool {
    matches!(
        run.status.as_str(),
        "failed" | "cancelled" | "partial" | "blocked" | "needs_intake" | "interrupted"
    )
}

fn pr_retryable_run(run: &crate::state::RunRecord) -> bool {
    run.status == "completed"
        && !is_synced(run)
        && run.pr_url.is_none()
        && matches!(run.pr_status.as_str(), "failed" | "missing")
}

fn is_synced(run: &crate::state::RunRecord) -> bool {
    matches!(
        run.sync_status.as_str(),
        "applied" | "synced" | "synced_locally"
    )
}

fn pr_creation_disabled(config: &ResolvedConfig) -> bool {
    matches!(config.pull_request_create.as_str(), "disabled" | "never")
}

fn local_review_without_pr(config: &ResolvedConfig, run: &crate::state::RunRecord) -> bool {
    pr_creation_disabled(config)
        && run.status == "completed"
        && run.pr_url.is_none()
        && !is_synced(run)
}

fn available_artifacts(config: &ResolvedConfig, paths: &RunArtifactPaths) -> Vec<String> {
    [
        &paths.summary,
        &paths.result,
        &paths.changeset,
        &paths.events,
    ]
    .into_iter()
    .filter(|path| path.exists())
    .map(|path| changeset_display_path(&config.repo_root, path))
    .collect()
}

fn validation_failure_message(result: &Value) -> Option<String> {
    let validation = result.get("validation")?;
    if let Some(commands) = validation.get("commands").and_then(Value::as_array) {
        for command in commands {
            let result = command.get("result").and_then(Value::as_str);
            if matches!(result, Some("fail" | "unavailable")) {
                let command_text = command
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or("validation command");
                return Some(format!(
                    "Validation `{command_text}` reported {}.",
                    result.unwrap_or("unknown")
                ));
            }
        }
    }
    validation
        .get("unavailable")
        .and_then(Value::as_str)
        .filter(|message| !message.trim().is_empty())
        .map(|message| format!("Validation unavailable: {message}"))
}

fn latest_event_message(events: &[Value]) -> Option<String> {
    events.iter().rev().find_map(|event| {
        let method = json_string(event, &["method"])?;
        let status = json_string(event, &["params", "turn", "status"])
            .or_else(|| json_string(event, &["result", "turn", "status"]))
            .or_else(|| json_string(event, &["params", "status"]));
        Some(match status {
            Some(status) => format!("{method} status {status}"),
            None => method.to_owned(),
        })
    })
}

fn latest_event_error(events: &[Value]) -> Option<String> {
    events.iter().rev().find_map(|event| {
        let explicit = json_string(event, &["params", "turn", "error", "message"])
            .or_else(|| json_string(event, &["result", "turn", "error", "message"]))
            .or_else(|| json_string(event, &["params", "error", "message"]))
            .or_else(|| json_string(event, &["error", "message"]))
            .or_else(|| json_string(event, &["params", "message"]))
            .or_else(|| json_string(event, &["error"]));
        if let Some(message) = explicit {
            return Some(message.to_owned());
        }
        let status = json_string(event, &["params", "turn", "status"])
            .or_else(|| json_string(event, &["result", "turn", "status"]));
        if matches!(status, Some("failed" | "interrupted" | "cancelled")) {
            return Some(format!(
                "{} ended with status {}.",
                json_string(event, &["method"]).unwrap_or("Codex turn"),
                status.unwrap_or("unknown")
            ));
        }
        let text = event.to_string();
        let lower = text.to_lowercase();
        if lower.contains("timeout") || lower.contains("timed out") {
            Some(compact_sentence(&text))
        } else {
            None
        }
    })
}

fn json_string<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

fn artifact_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact")
        .to_owned()
}

fn compact_sentence(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > 240 {
        compact.chars().take(237).collect::<String>() + "..."
    } else {
        compact
    }
}

fn review_next_action(
    config: &ResolvedConfig,
    run: &crate::state::RunRecord,
    has_summary: bool,
    has_result: bool,
) -> String {
    if matches!(
        run.status.as_str(),
        "failed" | "cancelled" | "partial" | "blocked"
    ) || !has_summary
        || !has_result
    {
        "Inspect run artifacts and retry when safe.".to_owned()
    } else if local_review_without_pr(config, run) {
        "Review local run artifacts and approve sync when ready.".to_owned()
    } else if run.pr_url.is_none() {
        "Create or retry the pull request for this run.".to_owned()
    } else {
        run.next_action.clone()
    }
}

fn start_path_story_id(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let story_id = path.strip_prefix("/api/tasks/")?.strip_suffix("/start")?;
    safe_identifier(story_id).then(|| story_id.to_owned())
}

fn recover_path_story_id(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let story_id = path.strip_prefix("/api/tasks/")?.strip_suffix("/recover")?;
    safe_identifier(story_id).then(|| story_id.to_owned())
}

fn retire_path_story_id(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let story_id = path.strip_prefix("/api/tasks/")?.strip_suffix("/retire")?;
    safe_identifier(story_id).then(|| story_id.to_owned())
}

fn context_path_story_id(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let story_id = path.strip_prefix("/api/tasks/")?.strip_suffix("/context")?;
    safe_identifier(story_id).then(|| story_id.to_owned())
}

fn traces_path_query(path: &str) -> Option<String> {
    if path == "/api/traces" {
        Some(String::new())
    } else {
        path.strip_prefix("/api/traces?").map(str::to_owned)
    }
}

fn events_path_run_id(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let run_id = path.strip_prefix("/api/runs/")?.strip_suffix("/events")?;
    safe_identifier(run_id).then(|| run_id.to_owned())
}

fn review_path_run_id(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let run_id = path.strip_prefix("/api/runs/")?.strip_suffix("/review")?;
    safe_identifier(run_id).then(|| run_id.to_owned())
}

fn sync_path_run_id(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let run_id = path.strip_prefix("/api/runs/")?.strip_suffix("/sync")?;
    safe_identifier(run_id).then(|| run_id.to_owned())
}

fn pr_merged_path_run_id(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let run_id = path
        .strip_prefix("/api/runs/")?
        .strip_suffix("/pr-merged")?;
    safe_identifier(run_id).then(|| run_id.to_owned())
}

fn pr_retry_path_run_id(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let run_id = path.strip_prefix("/api/runs/")?.strip_suffix("/pr-retry")?;
    safe_identifier(run_id).then(|| run_id.to_owned())
}

fn request_changes_path_run_id(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let run_id = path
        .strip_prefix("/api/runs/")?
        .strip_suffix("/request-changes")?;
    safe_identifier(run_id).then(|| run_id.to_owned())
}

fn feedback_path_parts(path: &str) -> Option<(String, String)> {
    let tail = path.strip_prefix("/api/runs/")?;
    let (run_id, filename) = tail.split_once("/feedback/")?;
    Some((run_id.to_owned(), filename.to_owned()))
}

fn generated_evidence_content_type(filename: &str) -> Option<&'static str> {
    let (stem, extension) = filename.rsplit_once('.')?;
    let number = stem.strip_prefix("evidence-")?;
    if number.len() != 2
        || !number.bytes().all(|byte| byte.is_ascii_digit())
        || !matches!(number.parse::<u8>().ok(), Some(1..=3))
    {
        return None;
    }
    match extension {
        "png" => Some("image/png"),
        "jpg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn context_response(config: &ResolvedConfig, story_id: &str) -> Result<HttpResponse, WebError> {
    let output = Command::new(harness_cli_path(config))
        .args(["context", "--story", story_id])
        .env("HARNESS_DB_PATH", &config.harness_db)
        .current_dir(&config.repo_root)
        .output()?;

    if !output.status.success() {
        return command_error_response(output);
    }

    json_response(
        200,
        &ContextResponse {
            story_id: story_id.to_owned(),
            content: String::from_utf8_lossy(&output.stdout).to_string(),
        },
    )
}

fn traces_response(config: &ResolvedConfig, query: &str) -> Result<HttpResponse, WebError> {
    use rusqlite::{params, Connection, Row};

    let story_id = match validated_identifier_query_param(query, "story_id") {
        Ok(value) => value,
        Err(error) => {
            return json_response(400, &ErrorResponse { error });
        }
    };
    let outcome = match validated_outcome_query_param(query) {
        Ok(value) => value,
        Err(error) => {
            return json_response(400, &ErrorResponse { error });
        }
    };
    let limit = query_param(query, "limit")
        .map(|value| {
            value
                .parse::<i64>()
                .ok()
                .filter(|parsed| *parsed > 0)
                .unwrap_or(50)
        })
        .unwrap_or(50)
        .min(200);
    let connection = Connection::open(&config.harness_db).map_err(sqlite_web_error)?;

    fn trace_item_from_row(row: &Row<'_>) -> rusqlite::Result<TraceItem> {
        Ok(TraceItem {
            id: row.get(0)?,
            story_id: row.get(1)?,
            summary: row.get(2)?,
            outcome: row
                .get::<_, Option<String>>(3)?
                .unwrap_or_else(|| "unknown".to_owned()),
            created_at: row.get(4)?,
            duration_seconds: row.get(5)?,
            harness_friction: row.get(6)?,
        })
    }

    let select = "SELECT id, story_id, task_summary, outcome, created_at, duration_seconds, harness_friction FROM trace";
    let traces = match (&story_id, &outcome) {
        (Some(story_id), Some(outcome)) => {
            let mut statement = connection
                .prepare(&format!(
                    "{select} WHERE story_id=?1 AND outcome=?2 ORDER BY id DESC LIMIT ?3"
                ))
                .map_err(sqlite_web_error)?;
            let rows = statement
                .query_map(params![story_id, outcome, limit], trace_item_from_row)
                .map_err(sqlite_web_error)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(sqlite_web_error)?
        }
        (Some(story_id), None) => {
            let mut statement = connection
                .prepare(&format!(
                    "{select} WHERE story_id=?1 ORDER BY id DESC LIMIT ?2"
                ))
                .map_err(sqlite_web_error)?;
            let rows = statement
                .query_map(params![story_id, limit], trace_item_from_row)
                .map_err(sqlite_web_error)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(sqlite_web_error)?
        }
        (None, Some(outcome)) => {
            let mut statement = connection
                .prepare(&format!(
                    "{select} WHERE outcome=?1 ORDER BY id DESC LIMIT ?2"
                ))
                .map_err(sqlite_web_error)?;
            let rows = statement
                .query_map(params![outcome, limit], trace_item_from_row)
                .map_err(sqlite_web_error)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(sqlite_web_error)?
        }
        (None, None) => {
            let mut statement = connection
                .prepare(&format!("{select} ORDER BY id DESC LIMIT ?1"))
                .map_err(sqlite_web_error)?;
            let rows = statement
                .query_map(params![limit], trace_item_from_row)
                .map_err(sqlite_web_error)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(sqlite_web_error)?
        }
    };
    let total = traces.len();

    json_response(200, &TraceResponse { traces, total })
}

fn tools_response(config: &ResolvedConfig) -> Result<HttpResponse, WebError> {
    let output = Command::new(harness_cli_path(config))
        .args(["query", "tools", "--json"])
        .env("HARNESS_DB_PATH", &config.harness_db)
        .current_dir(&config.repo_root)
        .output()?;

    if !output.status.success() {
        return command_error_response(output);
    }

    let tools = serde_json::from_slice::<Vec<ToolItem>>(&output.stdout)?;
    json_response(200, &ToolsResponse { tools })
}

fn tools_check_response(config: &ResolvedConfig) -> Result<HttpResponse, WebError> {
    let output = Command::new(harness_cli_path(config))
        .args(["tool", "check", "--json"])
        .env("HARNESS_DB_PATH", &config.harness_db)
        .current_dir(&config.repo_root)
        .output()?;

    if !output.status.success() {
        return command_error_response(output);
    }

    let tools = serde_json::from_slice::<serde_json::Value>(&output.stdout)?;
    json_response(200, &serde_json::json!({ "tools": tools }))
}

fn harness_cli_path(config: &ResolvedConfig) -> PathBuf {
    config
        .repo_root
        .join("scripts")
        .join("bin")
        .join(if cfg!(windows) {
            "harness-cli.exe"
        } else {
            "harness-cli"
        })
}

fn command_error_response(output: std::process::Output) -> Result<HttpResponse, WebError> {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    json_response(
        500,
        &ErrorResponse {
            error: if stderr.is_empty() { stdout } else { stderr },
        },
    )
}

fn query_param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name && !value.is_empty()).then(|| value.to_owned())
    })
}

fn validated_identifier_query_param(query: &str, name: &str) -> Result<Option<String>, String> {
    match query_param(query, name) {
        Some(value) if safe_identifier(&value) => Ok(Some(value)),
        Some(_) => Err(format!("invalid {name}")),
        None => Ok(None),
    }
}

fn validated_outcome_query_param(query: &str) -> Result<Option<String>, String> {
    match query_param(query, "outcome") {
        Some(value)
            if matches!(
                value.as_str(),
                "completed" | "blocked" | "partial" | "failed"
            ) =>
        {
            Ok(Some(value))
        }
        Some(_) => Err("invalid outcome".to_owned()),
        None => Ok(None),
    }
}

fn sqlite_web_error(error: rusqlite::Error) -> WebError {
    WebError::Io(std::io::Error::other(error))
}

fn json_response<T: Serialize>(status: u16, body: &T) -> Result<HttpResponse, WebError> {
    let status_text = match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    };
    let body = serde_json::to_vec(body)?;
    let mut response = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(&body);
    Ok(HttpResponse::new(response))
}

fn binary_response(
    status: u16,
    content_type: &str,
    body: Vec<u8>,
) -> Result<HttpResponse, WebError> {
    let status_text = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Internal Server Error",
    };
    let mut response = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(&body);
    Ok(HttpResponse::new(response))
}

fn static_response(config: &ResolvedConfig, request_path: &str) -> Result<HttpResponse, WebError> {
    let dist_dir = web_dist_dir(config);
    if !dist_dir.exists() {
        if request_path != "/" {
            return json_response(
                404,
                &ErrorResponse {
                    error: "not found".to_owned(),
                },
            );
        }
        return json_response(
            503,
            &ErrorResponse {
                error: "web UI assets are not built; run npm --prefix crates/harness-symphony/web-ui run build".to_owned(),
            },
        );
    }

    let asset_path = resolve_asset_path(&dist_dir, request_path)?;
    if !asset_path.exists() || !asset_path.is_file() {
        return json_response(
            404,
            &ErrorResponse {
                error: "not found".to_owned(),
            },
        );
    }

    let body = fs::read(&asset_path)?;
    let content_type = content_type(&asset_path);
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(&body);
    Ok(HttpResponse::new(response))
}

fn web_dist_dir(config: &ResolvedConfig) -> PathBuf {
    web_dist_dir_with_override(config, std::env::var_os(WEB_DIST_DIR_ENV))
}

fn web_dist_dir_with_override(
    config: &ResolvedConfig,
    override_path: Option<std::ffi::OsString>,
) -> PathBuf {
    if let Some(path) = override_path {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    config
        .repo_root
        .join("crates")
        .join("harness-symphony")
        .join("web-ui")
        .join("dist")
}

fn resolve_asset_path(dist_dir: &Path, request_path: &str) -> Result<PathBuf, WebError> {
    let path = request_path.split('?').next().unwrap_or(request_path);
    let relative = if path == "/" {
        PathBuf::from("index.html")
    } else {
        let trimmed = path.trim_start_matches('/');
        if trimmed.split('/').any(|segment| segment == "..") {
            return Err(WebError::InvalidAssetPath);
        }
        PathBuf::from(trimmed)
    };
    Ok(dist_dir.join(relative))
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("wasm") => "application/wasm",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("json") => "application/json; charset=utf-8",
        Some("html") | None => "text/html; charset=utf-8",
        _ => "application/octet-stream",
    }
}

impl BoardItemResponse {
    fn from_item(config: &ResolvedConfig, item: BoardItem) -> Self {
        let run = item.run_id.as_deref().and_then(|run_id| {
            RunStateStore::new(config.state_db.clone())
                .show_run(run_id)
                .ok()
        });
        let local_review = run
            .as_ref()
            .is_some_and(|run| local_review_without_pr(config, run));
        let board_state = if local_review && item.board_state == BoardState::NeedsAttention {
            BoardState::Review.label().to_owned()
        } else {
            item.board_state.label().to_owned()
        };
        let effective_board_state =
            if local_review && item.board_state == BoardState::NeedsAttention {
                BoardState::Review
            } else {
                item.board_state.clone()
            };
        let failure_summary = run
            .as_ref()
            .and_then(|run| failure_summary_for_run(config, run));
        let recovery_action = run.as_ref().and_then(|run| {
            recovery_action_for_run(&item.id, &item.story_status, &effective_board_state, run)
        });
        let reason = failure_summary
            .as_ref()
            .map(|summary| summary.reason.clone())
            .unwrap_or_else(|| {
                if local_review {
                    "review local run artifacts".to_owned()
                } else {
                    item.reason.clone()
                }
            });
        Self {
            id: item.id,
            title: item.title,
            board_state,
            story_status: item.story_status,
            lane: item.lane,
            verify: item.verify,
            blockers: item.blockers,
            unblocks: item.unblocks,
            parent_id: item.parent_id,
            children: item.children,
            hierarchy_depth: item.hierarchy_depth,
            run_id: item.run_id,
            active_run: item.active_run,
            reason,
            failure_summary,
            recovery_action,
        }
    }
}

impl From<SyncChange> for SyncChangeResponse {
    fn from(change: SyncChange) -> Self {
        Self {
            id: change.id,
            path: change.path.display().to_string(),
            applied: change.applied,
            operations: change.operations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SymphonyConfig;
    use rusqlite::{params, Connection};
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::process::Command;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn web_auto_open_uses_resolved_listener_url() {
        let opened_url = RefCell::new(None);
        let listener = prepare_web_server(
            WebServerOptions {
                host: "127.0.0.1".to_owned(),
                port: 0,
                open_browser: true,
            },
            |url| {
                opened_url.replace(Some(url.to_owned()));
                Ok::<_, String>(())
            },
        )
        .unwrap();

        let expected = format!("http://{}", listener.local_addr().unwrap());
        assert_eq!(opened_url.borrow().as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn web_auto_open_skips_launcher_when_disabled() {
        let called = Cell::new(false);
        let listener = prepare_web_server(
            WebServerOptions {
                host: "127.0.0.1".to_owned(),
                port: 0,
                open_browser: false,
            },
            |_| {
                called.set(true);
                Ok::<_, String>(())
            },
        )
        .unwrap();

        assert_ne!(listener.local_addr().unwrap().port(), 0);
        assert!(!called.get());
    }

    #[test]
    fn web_auto_open_failure_keeps_listener_available() {
        let listener = prepare_web_server(
            WebServerOptions {
                host: "127.0.0.1".to_owned(),
                port: 0,
                open_browser: true,
            },
            |_| Err("no browser available"),
        )
        .unwrap();

        assert_ne!(listener.local_addr().unwrap().port(), 0);
        assert_eq!(
            browser_open_warning("http://127.0.0.1:4317", "no browser available"),
            "warning: could not open Symphony Web UI at http://127.0.0.1:4317: no browser available. Open the URL manually."
        );
    }

    #[test]
    fn web_auto_open_maps_unspecified_ipv4_to_loopback() {
        let opened_url = RefCell::new(None);
        prepare_web_server(
            WebServerOptions {
                host: "0.0.0.0".to_owned(),
                port: 0,
                open_browser: true,
            },
            |url| {
                opened_url.replace(Some(url.to_owned()));
                Ok::<_, String>(())
            },
        )
        .unwrap();

        assert!(opened_url
            .borrow()
            .as_deref()
            .unwrap()
            .starts_with("http://127.0.0.1:"));
    }

    #[test]
    fn web_auto_open_supports_unspecified_ipv6_and_uses_loopback_url() {
        let opened_url = RefCell::new(None);
        prepare_web_server(
            WebServerOptions {
                host: "::".to_owned(),
                port: 0,
                open_browser: true,
            },
            |url| {
                opened_url.replace(Some(url.to_owned()));
                Ok::<_, String>(())
            },
        )
        .unwrap();

        assert!(opened_url
            .borrow()
            .as_deref()
            .unwrap()
            .starts_with("http://[::1]:"));
    }

    fn test_config(temp_dir: &tempfile::TempDir) -> ResolvedConfig {
        SymphonyConfig::default().resolve(temp_dir.path())
    }

    fn seed_story(db_path: &std::path::Path) {
        seed_story_with_status(db_path, "US-WEB", "Web backend", "planned");
    }

    fn seed_story_with_status(
        db_path: &std::path::Path,
        story_id: &str,
        title: &str,
        status: &str,
    ) {
        let connection = Connection::open(db_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE story (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    status TEXT NOT NULL,
                    risk_lane TEXT NOT NULL,
                    verify_command TEXT
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO story (id, title, status, risk_lane, verify_command)
                 VALUES (?1, ?2, ?3, 'normal', 'cargo test');",
                params![story_id, title, status],
            )
            .unwrap();
    }

    fn seed_runnable_story(db_path: &std::path::Path, story_id: &str) {
        let connection = Connection::open(db_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE story (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    status TEXT NOT NULL,
                    risk_lane TEXT NOT NULL,
                    verify_command TEXT
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO story (id, title, status, risk_lane, verify_command)
                 VALUES (?1, ?2, 'planned', 'normal', 'cargo test');",
                params![story_id, "Runnable"],
            )
            .unwrap();
    }

    fn init_git_repo(path: &std::path::Path) {
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
        let add = Command::new("git")
            .args(["add", "README.md"])
            .current_dir(path)
            .output()
            .unwrap();
        assert!(add.status.success());
        let commit = Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()
            .unwrap();
        assert!(commit.status.success());
    }

    fn add_test_run(config: &ResolvedConfig, run_id: &str, status: &str) {
        RunStateStore::new(config.state_db.clone())
            .add_run(crate::state::NewRunRecord {
                run_id: run_id.to_owned(),
                story_id: "US-ATTN".to_owned(),
                branch: Some(format!("symphony/{run_id}")),
                worktree: config.repo_root.join(".symphony/worktrees").join(run_id),
                lightweight: false,
                status: status.to_owned(),
                result_path: Some(PathBuf::from(format!(".harness/runs/{run_id}/RESULT.json"))),
                sync_status: "not_applied".to_owned(),
                next_action: "inspect run".to_owned(),
            })
            .unwrap();
    }

    fn add_story_run(config: &ResolvedConfig, run_id: &str, story_id: &str, status: &str) {
        RunStateStore::new(config.state_db.clone())
            .add_run(crate::state::NewRunRecord {
                run_id: run_id.to_owned(),
                story_id: story_id.to_owned(),
                branch: Some(format!("symphony/{run_id}")),
                worktree: config.repo_root.join(".symphony/worktrees").join(run_id),
                lightweight: false,
                status: status.to_owned(),
                result_path: Some(PathBuf::from(format!(".harness/runs/{run_id}/RESULT.json"))),
                sync_status: "not_applied".to_owned(),
                next_action: "review run result".to_owned(),
            })
            .unwrap();
    }

    fn request_changes_http_request(
        run_id: &str,
        reason: &str,
        evidence: &[(&str, &str, &[u8])],
    ) -> crate::upload::HttpRequest {
        let boundary = "request-changes-boundary";
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"reason\"\r\n\r\n");
        body.extend_from_slice(reason.as_bytes());
        body.extend_from_slice(b"\r\n");
        for (filename, content_type, bytes) in evidence {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"evidence\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(bytes);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        let mut request = format!(
            "POST /api/runs/{run_id}/request-changes HTTP/1.1\r\nContent-Type: multipart/form-data; boundary={boundary}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        request.extend_from_slice(&body);
        crate::upload::parse_http_request(&request).unwrap()
    }

    fn test_run_record(status: &str) -> crate::state::RunRecord {
        crate::state::RunRecord {
            run_id: format!("run_{status}"),
            story_id: "US-ATTN".to_owned(),
            branch: Some(format!("symphony/run_{status}")),
            worktree: PathBuf::from(format!(".symphony/worktrees/run_{status}")),
            lightweight: false,
            status: status.to_owned(),
            result_path: Some(PathBuf::from(format!(
                ".harness/runs/run_{status}/RESULT.json"
            ))),
            pr_url: None,
            pr_status: "missing".to_owned(),
            sync_status: "not_applied".to_owned(),
            next_action: "inspect run".to_owned(),
            agent: "codex".to_owned(),
        }
    }

    fn write_summary(config: &ResolvedConfig, run_id: &str) {
        let run_dir = config.runs_dir.join(run_id);
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(run_dir.join("SUMMARY.md"), "# Summary\n\nNeeds review.\n").unwrap();
    }

    fn write_result(config: &ResolvedConfig, run_id: &str, body: &str) {
        let run_dir = config.runs_dir.join(run_id);
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(run_dir.join("RESULT.json"), body).unwrap();
    }

    fn write_events(config: &ResolvedConfig, run_id: &str, body: &str) {
        let run_dir = config.runs_dir.join(run_id);
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(run_dir.join("APP_SERVER_EVENTS.jsonl"), body).unwrap();
    }

    #[cfg(unix)]
    fn make_executable(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &std::path::Path) {}

    fn write_fake_harness_cli(config: &ResolvedConfig) {
        let bin_dir = config.repo_root.join("scripts").join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let script = bin_dir.join(if cfg!(windows) {
            "harness-cli.exe"
        } else {
            "harness-cli"
        });
        fs::write(
            &script,
            r#"#!/usr/bin/env sh
if [ "$1" = "context" ]; then
  printf '# Context\n\nStory %s ready.\n' "$3"
  exit 0
fi
if [ "$1" = "query" ] && [ "$2" = "tools" ]; then
  printf '[{"provider":"builtin","name":"query tools","command":"query tools","description":"Show tool entries.","responsibility":"Tool access","source":"compiled","since":"built-in","kind":"cli","capability":"tool-access","scan_target":null,"status":"present","checked_at":null}]'
  exit 0
fi
if [ "$1" = "tool" ] && [ "$2" = "check" ]; then
  printf '[{"name":"query tools","kind":"cli","capability":"tool-access","status":"present","detail":"ok"}]'
  exit 0
fi
printf 'unsupported harness-cli call\n' >&2
exit 1
"#,
        )
        .unwrap();
        make_executable(&script);
    }

    fn seed_trace_table(db_path: &std::path::Path) {
        let connection = Connection::open(db_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE trace (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    task_summary TEXT NOT NULL,
                    intake_id INTEGER,
                    story_id TEXT,
                    agent TEXT,
                    actions_taken TEXT,
                    files_read TEXT,
                    files_changed TEXT,
                    decisions_made TEXT,
                    errors TEXT,
                    outcome TEXT,
                    duration_seconds INTEGER,
                    token_estimate INTEGER,
                    harness_friction TEXT,
                    notes TEXT
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO trace (task_summary, story_id, outcome, duration_seconds, harness_friction)
                 VALUES (?1, ?2, ?3, ?4, ?5);",
                params![
                    "Trace target",
                    "US-TRACE",
                    "completed",
                    12_i64,
                    "manual review needed"
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO trace (task_summary, story_id, outcome)
                 VALUES (?1, ?2, ?3);",
                params!["Trace other", "US-OTHER", "failed"],
            )
            .unwrap();
    }

    #[test]
    fn health_request_returns_ok_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);

        let response = handle_request(&config, "GET /health HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.ends_with(r#"{"ok":true}"#));
    }

    #[test]
    fn request_changes_binary_http_request_routes_without_string_conversion() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let request = crate::upload::parse_http_request(b"GET /health HTTP/1.1\r\n\r\n").unwrap();

        let response = handle_http_request(&config, &request).unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.ends_with(r#"{"ok":true}"#));
    }

    #[test]
    fn request_changes_creates_replacement_and_preserves_source() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = test_config(&temp_dir);
        config.pull_request_create = "disabled".to_owned();
        init_git_repo(temp_dir.path());
        seed_runnable_story(&config.harness_db, "US-084");
        add_story_run(&config, "run_old", "US-084", "completed");
        let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1];
        let request = request_changes_http_request(
            "run_old",
            "Fix spacing",
            &[("proof.png", "image/png", &png)],
        );

        let response =
            request_changes_response_with_spawn(&config, "run_old", &request, |_, _| {}).unwrap();

        assert!(response.starts_with("HTTP/1.1 202 Accepted"));
        let body: Value = serde_json::from_slice(response.body()).unwrap();
        let replacement_run_id = body["run_id"].as_str().unwrap();
        let store = RunStateStore::new(config.state_db.clone());
        assert_eq!(store.show_run("run_old").unwrap().status, "rejected");
        assert_eq!(
            store.show_run(replacement_run_id).unwrap().story_id,
            "US-084"
        );
        assert_eq!(
            body["feedback"]["evidence_paths"].as_array().unwrap().len(),
            1
        );
    }

    #[test]
    fn request_changes_invalid_image_leaves_source_completed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = test_config(&temp_dir);
        config.pull_request_create = "disabled".to_owned();
        init_git_repo(temp_dir.path());
        seed_runnable_story(&config.harness_db, "US-084");
        add_story_run(&config, "run_old", "US-084", "completed");
        let request = request_changes_http_request(
            "run_old",
            "Fix spacing",
            &[("proof.png", "image/png", b"not png")],
        );

        let response = handle_http_request(&config, &request).unwrap();

        assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
        let store = RunStateStore::new(config.state_db.clone());
        assert_eq!(store.show_run("run_old").unwrap().status, "completed");
        assert_eq!(store.list_runs().unwrap().len(), 1);
    }

    #[test]
    fn request_changes_refuses_done_stale_and_active_conflicts() {
        let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

        let done_dir = tempfile::tempdir().unwrap();
        let done_config = test_config(&done_dir);
        init_git_repo(done_dir.path());
        seed_story_with_status(
            &done_config.harness_db,
            "US-084",
            "Done story",
            "implemented",
        );
        add_story_run(&done_config, "run_done", "US-084", "completed");
        let done_request =
            request_changes_http_request("run_done", "Try again", &[("a.png", "image/png", &png)]);
        let done =
            request_changes_response_with_spawn(&done_config, "run_done", &done_request, |_, _| {})
                .unwrap();
        assert!(done.starts_with("HTTP/1.1 409 Conflict"));

        let stale_dir = tempfile::tempdir().unwrap();
        let mut stale_config = test_config(&stale_dir);
        stale_config.pull_request_create = "disabled".to_owned();
        init_git_repo(stale_dir.path());
        seed_runnable_story(&stale_config.harness_db, "US-084");
        add_story_run(&stale_config, "run_old", "US-084", "completed");
        add_story_run(&stale_config, "run_znew", "US-084", "completed");
        let stale_request =
            request_changes_http_request("run_old", "Try again", &[("a.png", "image/png", &png)]);
        let stale = request_changes_response_with_spawn(
            &stale_config,
            "run_old",
            &stale_request,
            |_, _| {},
        )
        .unwrap();
        assert!(stale.starts_with("HTTP/1.1 409 Conflict"));

        let active_dir = tempfile::tempdir().unwrap();
        let mut active_config = test_config(&active_dir);
        active_config.pull_request_create = "disabled".to_owned();
        init_git_repo(active_dir.path());
        seed_runnable_story(&active_config.harness_db, "US-084");
        add_story_run(&active_config, "run_old", "US-084", "completed");
        add_story_run(&active_config, "run_active", "US-ACTIVE", "prepared");
        let active_request =
            request_changes_http_request("run_old", "Try again", &[("a.png", "image/png", &png)]);
        let active = request_changes_response_with_spawn(
            &active_config,
            "run_old",
            &active_request,
            |_, _| {},
        )
        .unwrap();
        assert!(active.starts_with("HTTP/1.1 409 Conflict"));
        assert_eq!(
            RunStateStore::new(active_config.state_db.clone())
                .show_run("run_old")
                .unwrap()
                .status,
            "completed"
        );
    }

    #[test]
    fn request_changes_review_metadata_and_scoped_evidence_are_safe() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = test_config(&temp_dir);
        config.pull_request_create = "disabled".to_owned();
        init_git_repo(temp_dir.path());
        seed_runnable_story(&config.harness_db, "US-084");
        add_story_run(&config, "run_old", "US-084", "completed");
        let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 4, 2];
        let request = request_changes_http_request(
            "run_old",
            "Fix spacing",
            &[("proof.png", "image/png", &png)],
        );
        let response =
            request_changes_response_with_spawn(&config, "run_old", &request, |_, _| {}).unwrap();
        let body: Value = serde_json::from_slice(response.body()).unwrap();
        let run_id = body["run_id"].as_str().unwrap();

        let review = handle_request(
            &config,
            &format!("GET /api/runs/{run_id}/review HTTP/1.1\r\n\r\n"),
        )
        .unwrap();
        assert!(review.contains(r#""reason":"Fix spacing""#));
        assert!(review.contains(&format!("/api/runs/{run_id}/feedback/evidence-01.png")));

        let evidence = handle_request(
            &config,
            &format!("GET /api/runs/{run_id}/feedback/evidence-01.png HTTP/1.1\r\n\r\n"),
        )
        .unwrap();
        assert!(evidence.starts_with("HTTP/1.1 200 OK"));
        assert_eq!(evidence.body(), png);

        let traversal = handle_request(
            &config,
            &format!("GET /api/runs/{run_id}/feedback/../RUN_CONTRACT.json HTTP/1.1\r\n\r\n"),
        )
        .unwrap();
        assert!(traversal.starts_with("HTTP/1.1 400 Bad Request"));
    }

    #[test]
    fn request_changes_review_ignores_missing_reason_artifact() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = test_config(&temp_dir);
        config.pull_request_create = "disabled".to_owned();
        init_git_repo(temp_dir.path());
        seed_runnable_story(&config.harness_db, "US-084");
        add_story_run(&config, "run_old", "US-084", "completed");
        let request = request_changes_http_request("run_old", "Fix spacing", &[]);
        let response =
            request_changes_response_with_spawn(&config, "run_old", &request, |_, _| {}).unwrap();
        let body: Value = serde_json::from_slice(response.body()).unwrap();
        let run_id = body["run_id"].as_str().unwrap();
        fs::remove_file(config.runs_dir.join(run_id).join("feedback/reason.md")).unwrap();

        let review = handle_request(
            &config,
            &format!("GET /api/runs/{run_id}/review HTTP/1.1\r\n\r\n"),
        )
        .unwrap();

        assert!(review.starts_with("HTTP/1.1 200 OK"));
        assert!(review.contains(r#""request_changes":null"#));
    }

    #[test]
    fn settings_default_agent_falls_back_to_config_adapter() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);

        let response = handle_request(&config, "GET /api/settings HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(&format!(r#""default_agent":"{}""#, config.agent_adapter)));
    }

    #[test]
    fn settings_update_persists_default_agent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);

        let update = handle_request(
            &config,
            "PUT /api/settings HTTP/1.1\r\n\r\n{\"default_agent\":\"opencode\"}",
        )
        .unwrap();
        assert!(update.starts_with("HTTP/1.1 200 OK"));

        let fetched = handle_request(&config, "GET /api/settings HTTP/1.1\r\n\r\n").unwrap();
        assert!(fetched.contains(r#""default_agent":"opencode""#));
    }

    #[test]
    fn settings_update_rejects_unknown_agent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);

        let response = handle_request(
            &config,
            "PUT /api/settings HTTP/1.1\r\n\r\n{\"default_agent\":\"clippy\"}",
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 400"));
        assert!(response.contains("codex, opencode"));
    }

    #[test]
    fn start_request_rejects_unknown_agent_without_touching_default() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        seed_story(&config.harness_db);

        let response = handle_request(
            &config,
            "POST /api/tasks/US-WEB/start HTTP/1.1\r\n\r\n{\"agent\":\"clippy\"}",
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 400"));
        let settings = handle_request(&config, "GET /api/settings HTTP/1.1\r\n\r\n").unwrap();
        assert!(settings.contains(&format!(r#""default_agent":"{}""#, config.agent_adapter)));
    }

    #[test]
    fn board_request_returns_board_items_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        seed_story(&config.harness_db);

        let response = handle_request(&config, "GET /api/board HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""id":"US-WEB""#));
        assert!(response.contains(r#""board_state":"Ready""#));
    }

    #[test]
    fn unsupported_path_returns_json_404() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);

        let response = handle_request(&config, "GET /nope HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 404 Not Found"));
        assert!(response.contains(r#""error":"not found""#));
    }

    #[test]
    fn run_start_path_extracts_story_id() {
        assert_eq!(
            start_path_story_id("/api/tasks/US-050/start"),
            Some("US-050".to_owned())
        );
        assert_eq!(start_path_story_id("/api/tasks/../start"), None);
        assert_eq!(
            retire_path_story_id("/api/tasks/US-064/retire"),
            Some("US-064".to_owned())
        );
        assert_eq!(retire_path_story_id("/api/tasks/../retire"), None);
    }

    #[test]
    fn retire_ready_task_updates_story_status_and_removes_it_from_board() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        seed_story_with_status(
            &config.harness_db,
            "US-064",
            "Ready Work Story Delete Action",
            "planned",
        );

        let response =
            handle_request(&config, "POST /api/tasks/US-064/retire HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""story_id":"US-064""#));
        assert!(response.contains(r#""status":"retired""#));
        let connection = Connection::open(&config.harness_db).unwrap();
        let status = connection
            .query_row("SELECT status FROM story WHERE id='US-064';", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(status, "retired");

        let board = handle_request(&config, "GET /api/board HTTP/1.1\r\n\r\n").unwrap();
        assert!(board.starts_with("HTTP/1.1 200 OK"));
        assert!(!board.contains("US-064"));
    }

    #[test]
    fn retire_task_refuses_non_ready_story_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        seed_story_with_status(&config.harness_db, "US-DONE", "Done Story", "implemented");

        let response =
            handle_request(&config, "POST /api/tasks/US-DONE/retire HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 409 Conflict"));
        assert!(response.contains("only Ready stories can be retired"));
    }

    #[test]
    fn guided_intake_create_writes_intake_and_story() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let connection = Connection::open(&config.harness_db).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE intake (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    input_type TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    risk_lane TEXT NOT NULL,
                    risk_flags TEXT,
                    affected_docs TEXT,
                    story_id TEXT,
                    notes TEXT
                );
                CREATE TABLE story (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'planned',
                    risk_lane TEXT NOT NULL,
                    contract_doc TEXT,
                    verify_command TEXT,
                    notes TEXT
                );",
            )
            .unwrap();
        let body = serde_json::json!({
            "idea": "Make review evidence easier to scan",
            "audience": "Maintainers reviewing local Symphony runs",
            "outcome": "They can approve or reject a run without opening raw artifacts first",
            "non_goals": "No automatic Symphony run start",
            "validation": "npm --prefix crates/harness-symphony/web-ui run e2e"
        })
        .to_string();
        let request = format!(
            "POST /api/intake HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = handle_request(&config, &request).unwrap();

        assert!(response.starts_with("HTTP/1.1 201 Created"));
        assert!(response.contains(r#""story_id":"US-001""#));
        assert!(response.contains(r#""status":"planned""#));
        let (intake_count, story_count): (i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM intake), (SELECT COUNT(*) FROM story);",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((intake_count, story_count), (1, 1));
        let notes = connection
            .query_row("SELECT notes FROM story WHERE id='US-001';", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        assert!(notes.contains("Audience: Maintainers reviewing local Symphony runs"));
        assert!(notes.contains("Non-goals: No automatic Symphony run start"));
    }

    #[test]
    fn events_request_returns_jsonl_events() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let run_dir = config.runs_dir.join("run_1");
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(
            run_dir.join("APP_SERVER_EVENTS.jsonl"),
            "{\"method\":\"turn/started\"}\nnot json\n{\"method\":\"turn/completed\"}\n",
        )
        .unwrap();

        let response =
            handle_request(&config, "GET /api/runs/run_1/events HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""run_id":"run_1""#));
        assert!(response.contains(r#""method":"turn/started""#));
        assert!(response.contains(r#""method":"turn/completed""#));
        assert!(!response.contains("not json"));
    }

    #[test]
    fn review_request_returns_run_artifacts_and_changeset_preview() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_review".to_owned(),
                story_id: "US-REVIEW".to_owned(),
                branch: Some("symphony/run_review".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_review/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "review pull request".to_owned(),
            })
            .unwrap();
        store
            .update_pr_url("run_review", "https://example.test/pr/1")
            .unwrap();
        let run_dir = config.runs_dir.join("run_review");
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(run_dir.join("SUMMARY.md"), "# Summary\n\nDone.\n").unwrap();
        fs::write(
            run_dir.join("RESULT.json"),
            r#"{"version":1,"run_id":"run_review","story_id":"US-REVIEW","outcome":"completed","changed_files":["src/lib.rs"],"validation":{"commands":[{"command":"cargo test","result":"pass"}]}}"#,
        )
        .unwrap();
        fs::write(
            run_dir.join("changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_review"}
{"op":"story.update","version":1,"id":"US-REVIEW","payload":{"status":"implemented"}}"#,
        )
        .unwrap();
        fs::write(
            run_dir.join("APP_SERVER_EVENTS.jsonl"),
            "{\"method\":\"turn/completed\"}\n",
        )
        .unwrap();

        let response =
            handle_request(&config, "GET /api/runs/run_review/review HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""story_id":"US-REVIEW""#));
        assert!(response.contains(r#""outcome":"completed""#));
        assert!(response.contains("Harness Changes"));
        assert!(response.contains("https://example.test/pr/1"));
        assert!(response.contains("src/lib.rs"));
        assert!(response.contains("turn/completed"));
    }

    #[test]
    fn review_request_summarizes_codex_timeout_failure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        add_test_run(&config, "run_timeout", "failed");
        write_summary(&config, "run_timeout");
        write_events(
            &config,
            "run_timeout",
            r#"{"method":"turn/completed","params":{"turn":{"status":"failed","error":{"message":"turn-state query timed out while waiting for Codex"}}}}"#,
        );

        let response =
            handle_request(&config, "GET /api/runs/run_timeout/review HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Codex app-server timeout"));
        assert!(response.contains("turn-state query timed out while waiting for Codex"));
        assert!(response.contains("APP_SERVER_EVENTS.jsonl"));
    }

    #[test]
    fn review_request_explains_missing_and_malformed_artifacts() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        add_test_run(&config, "run_missing", "failed");
        write_summary(&config, "run_missing");

        let missing =
            handle_request(&config, "GET /api/runs/run_missing/review HTTP/1.1\r\n\r\n").unwrap();

        assert!(missing.starts_with("HTTP/1.1 200 OK"));
        assert!(missing.contains("Missing artifact"));
        assert!(missing.contains("RESULT.json is missing"));

        add_test_run(&config, "run_malformed", "failed");
        write_summary(&config, "run_malformed");
        write_result(&config, "run_malformed", "{not json");

        let malformed = handle_request(
            &config,
            "GET /api/runs/run_malformed/review HTTP/1.1\r\n\r\n",
        )
        .unwrap();

        assert!(malformed.starts_with("HTTP/1.1 200 OK"));
        assert!(malformed.contains("Malformed artifact"));
        assert!(malformed.contains("RESULT.json is malformed"));
    }

    #[test]
    fn review_request_explains_result_story_mismatch() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        add_test_run(&config, "run_mismatch", "failed");
        write_summary(&config, "run_mismatch");
        write_result(
            &config,
            "run_mismatch",
            r#"{"version":1,"run_id":"run_mismatch","story_id":"US-WRONG","outcome":"completed","validation":{"commands":[{"command":"cargo test","result":"pass"}]}}"#,
        );

        let response = handle_request(
            &config,
            "GET /api/runs/run_mismatch/review HTTP/1.1\r\n\r\n",
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Invalid result artifact"));
        assert!(response.contains("RESULT.json story_id mismatch"));
        assert!(response.contains("US-ATTN"));
        assert!(response.contains("US-WRONG"));
    }

    #[test]
    fn review_request_summarizes_pr_and_validation_failures() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let store = RunStateStore::new(config.state_db.clone());
        add_test_run(&config, "run_pr_fail", "completed");
        store
            .record_pr_failure("run_pr_fail", "gh auth failed")
            .unwrap();
        write_summary(&config, "run_pr_fail");
        write_result(
            &config,
            "run_pr_fail",
            r#"{"version":1,"run_id":"run_pr_fail","story_id":"US-ATTN","outcome":"completed","validation":{"commands":[{"command":"cargo test","result":"pass"}]}}"#,
        );

        let pr_failure =
            handle_request(&config, "GET /api/runs/run_pr_fail/review HTTP/1.1\r\n\r\n").unwrap();

        assert!(pr_failure.contains("PR creation failure"));
        assert!(pr_failure.contains("gh auth failed"));

        add_test_run(&config, "run_validation", "completed");
        write_summary(&config, "run_validation");
        write_result(
            &config,
            "run_validation",
            r#"{"version":1,"run_id":"run_validation","story_id":"US-ATTN","outcome":"failed","validation":{"commands":[{"command":"npm test","result":"fail"}]}}"#,
        );

        let validation = handle_request(
            &config,
            "GET /api/runs/run_validation/review HTTP/1.1\r\n\r\n",
        )
        .unwrap();

        assert!(validation.contains("Validation failure"));
        assert!(validation.contains("Validation `npm test` reported fail."));
    }

    #[test]
    fn recovery_action_marks_retryable_execution_and_pr_failures() {
        for status in [
            "failed",
            "cancelled",
            "partial",
            "blocked",
            "needs_intake",
            "interrupted",
        ] {
            let run = test_run_record(status);
            let action =
                recovery_action_for_run("US-ATTN", "planned", &BoardState::NeedsAttention, &run)
                    .unwrap();
            assert_eq!(action.kind, "execution_retry");
            assert!(action.endpoint.ends_with("/api/tasks/US-ATTN/recover"));
        }

        let mut pr_run = test_run_record("completed");
        pr_run.pr_status = "failed".to_owned();
        let action =
            recovery_action_for_run("US-ATTN", "planned", &BoardState::NeedsAttention, &pr_run)
                .unwrap();
        assert_eq!(action.kind, "pr_retry");
        assert!(action
            .endpoint
            .ends_with("/api/runs/run_completed/pr-retry"));

        let mut review_run = test_run_record("completed");
        review_run.pr_url = Some("https://example.test/pr/1".to_owned());
        review_run.pr_status = "created".to_owned();
        assert!(
            recovery_action_for_run("US-ATTN", "planned", &BoardState::Review, &review_run)
                .is_none()
        );

        let implemented_run = test_run_record("failed");
        assert!(recovery_action_for_run(
            "US-ATTN",
            "implemented",
            &BoardState::NeedsAttention,
            &implemented_run
        )
        .is_none());
    }

    #[test]
    fn review_request_explains_malformed_event_log() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        add_test_run(&config, "run_bad_events", "failed");
        write_summary(&config, "run_bad_events");
        write_result(
            &config,
            "run_bad_events",
            r#"{"version":1,"run_id":"run_bad_events","story_id":"US-ATTN","outcome":"failed","validation":{"commands":[{"command":"cargo test","result":"pass"}]}}"#,
        );
        write_events(
            &config,
            "run_bad_events",
            "not json\n{\"method\":\"thread/started\"}\n",
        );

        let response = handle_request(
            &config,
            "GET /api/runs/run_bad_events/review HTTP/1.1\r\n\r\n",
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Malformed event log"));
        assert!(response.contains("APP_SERVER_EVENTS.jsonl has 1 malformed JSON line"));
    }

    #[test]
    fn sync_request_returns_not_found_for_unknown_run() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);

        let response =
            handle_request(&config, "POST /api/runs/run_missing/sync HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 404 Not Found"));
        assert!(response.contains("run not found"));
    }

    #[test]
    fn pr_merged_endpoint_marks_review_run_merged() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_merge".to_owned(),
                story_id: "US-MERGE".to_owned(),
                branch: Some("symphony/run_merge".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_merge/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "review pull request".to_owned(),
            })
            .unwrap();
        store
            .update_pr_url("run_merge", "https://example.test/pr/1")
            .unwrap();

        let response = handle_request(
            &config,
            "POST /api/runs/run_merge/pr-merged HTTP/1.1\r\n\r\n",
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""pr_status":"merged""#));
        assert_eq!(store.show_run("run_merge").unwrap().pr_status, "merged");
    }

    #[test]
    fn recover_request_prepares_new_run_without_rewriting_failed_run() {
        let temp_dir = tempfile::tempdir().unwrap();
        init_git_repo(temp_dir.path());
        let mut config = test_config(&temp_dir);
        config.agent_adapter = "custom".to_owned();
        config.agent_command = vec!["sh".to_owned(), "-c".to_owned(), "sleep 1".to_owned()];
        seed_runnable_story(&config.harness_db, "US-ATTN");
        add_test_run(&config, "run_failed", "failed");

        let response =
            handle_request(&config, "POST /api/tasks/US-ATTN/recover HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 202 Accepted"));
        assert!(response.contains(r#""story_id":"US-ATTN""#));
        assert_eq!(
            RunStateStore::new(config.state_db.clone())
                .show_run("run_failed")
                .unwrap()
                .status,
            "failed"
        );
        let runs = RunStateStore::new(config.state_db.clone())
            .list_runs()
            .unwrap();
        assert!(runs.iter().any(|run| run.run_id == "run_failed"));
        assert!(runs
            .iter()
            .any(|run| run.story_id == "US-ATTN" && run.run_id != "run_failed"));
    }

    #[test]
    fn recover_request_refuses_review_done_and_active_conflicts() {
        let temp_dir = tempfile::tempdir().unwrap();
        init_git_repo(temp_dir.path());
        let config = test_config(&temp_dir);
        seed_runnable_story(&config.harness_db, "US-ATTN");
        add_test_run(&config, "run_review", "completed");
        let store = RunStateStore::new(config.state_db.clone());
        store
            .update_pr_url("run_review", "https://example.test/pr/1")
            .unwrap();

        let review =
            handle_request(&config, "POST /api/tasks/US-ATTN/recover HTTP/1.1\r\n\r\n").unwrap();
        assert!(review.starts_with("HTTP/1.1 409 Conflict"));
        assert!(review.contains("not recoverable"));

        let temp_dir = tempfile::tempdir().unwrap();
        init_git_repo(temp_dir.path());
        let config = test_config(&temp_dir);
        seed_runnable_story(&config.harness_db, "US-ATTN");
        add_test_run(&config, "run_failed", "failed");
        RunStateStore::new(config.state_db.clone())
            .add_run(crate::state::NewRunRecord {
                run_id: "run_active".to_owned(),
                story_id: "US-OTHER".to_owned(),
                branch: Some("symphony/run_active".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "running".to_owned(),
                result_path: None,
                sync_status: "not_applied".to_owned(),
                next_action: "wait".to_owned(),
            })
            .unwrap();

        let active =
            handle_request(&config, "POST /api/tasks/US-ATTN/recover HTTP/1.1\r\n\r\n").unwrap();
        assert!(active.starts_with("HTTP/1.1 409 Conflict"));
        assert!(active.contains("active run already exists"));
    }

    #[test]
    fn pr_retry_endpoint_reuses_pr_creation_and_updates_review_state() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        init_git_repo(temp_dir.path());
        let origin = temp_dir.path().join("origin.git");
        let output = Command::new("git")
            .args(["init", "--bare"])
            .arg(&origin)
            .output()
            .unwrap();
        assert!(output.status.success());
        let output = Command::new("git")
            .args(["remote", "add", "origin"])
            .arg(&origin)
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        assert!(output.status.success());
        let worktree = temp_dir.path().join(".symphony/worktrees/run_pr_retry");
        fs::create_dir_all(worktree.parent().unwrap()).unwrap();
        let output = Command::new("git")
            .args(["worktree", "add", "-b", "symphony/run_pr_retry"])
            .arg(&worktree)
            .arg("HEAD")
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        assert!(output.status.success());

        let mut config = test_config(&temp_dir);
        config.pull_request_create = "ask".to_owned();
        seed_runnable_story(&config.harness_db, "US-PR");
        fs::create_dir_all(config.runs_dir.join("run_pr_retry")).unwrap();
        fs::write(
            config.runs_dir.join("run_pr_retry/SUMMARY.md"),
            "# Summary\n\nReady for PR.\n",
        )
        .unwrap();
        fs::write(
            config.runs_dir.join("run_pr_retry/RESULT.json"),
            r#"{"version":1,"run_id":"run_pr_retry","story_id":"US-PR","outcome":"completed","validation":{"commands":[{"command":"cargo test","result":"pass"}]}}"#,
        )
        .unwrap();
        fs::create_dir_all(worktree.join(".harness/changesets")).unwrap();
        fs::write(
            worktree.join(".harness/changesets/run_pr_retry.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_pr_retry"}
{"op":"story.update","version":1,"id":"US-PR","payload":{"status":"implemented"}}"#,
        )
        .unwrap();
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_pr_retry".to_owned(),
                story_id: "US-PR".to_owned(),
                branch: Some("symphony/run_pr_retry".to_owned()),
                worktree: worktree.clone(),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_pr_retry/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "retry pull request creation".to_owned(),
            })
            .unwrap();
        store
            .record_pr_failure("run_pr_retry", "gh auth failed")
            .unwrap();

        let bin_dir = temp_dir.path().join("fake-bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let gh = bin_dir.join("gh");
        fs::write(&gh, "#!/bin/sh\necho https://example.test/pr/67\n").unwrap();
        make_executable(&gh);
        let old_path = std::env::var_os("PATH");
        let new_path = format!(
            "{}:{}",
            bin_dir.display(),
            old_path
                .as_ref()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
        );
        std::env::set_var("PATH", new_path);

        let response = handle_request(
            &config,
            "POST /api/runs/run_pr_retry/pr-retry HTTP/1.1\r\n\r\n",
        )
        .unwrap();

        if let Some(path) = old_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""pr_status":"created""#));
        assert!(response.contains("https://example.test/pr/67"));
        let updated = store.show_run("run_pr_retry").unwrap();
        assert_eq!(updated.pr_status, "created");
        assert_eq!(
            updated.pr_url.as_deref(),
            Some("https://example.test/pr/67")
        );
    }

    #[test]
    fn disabled_pr_creation_leaves_completed_run_ready_for_local_review() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = test_config(&temp_dir);
        config.pull_request_create = "disabled".to_owned();
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_local_review".to_owned(),
                story_id: "US-LOCAL-REVIEW".to_owned(),
                branch: Some("symphony/run_local_review".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_local_review/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "review run result".to_owned(),
            })
            .unwrap();

        create_review_pr(&config, "run_local_review").unwrap();

        let run = store.show_run("run_local_review").unwrap();
        assert_eq!(run.status, "completed");
        assert_eq!(run.pr_status, "not_applicable");
        assert_eq!(run.next_action, "review local run artifacts");
        assert!(!run_needs_attention(&config, &run));
        assert_eq!(
            review_next_action(&config, &run, true, true),
            "Review local run artifacts and approve sync when ready."
        );
    }

    #[test]
    fn sync_request_allows_completed_local_run_when_pr_creation_is_disabled() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = test_config(&temp_dir);
        config.pull_request_create = "disabled".to_owned();
        fs::create_dir_all(&config.changeset_directory).unwrap();
        fs::create_dir_all(temp_dir.path().join("scripts/bin")).unwrap();
        fs::write(
            config
                .changeset_directory
                .join("run_local_sync.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_local_sync"}
{"op":"story.update","version":1,"id":"US-LOCAL-SYNC","payload":{"status":"implemented"}}
"#,
        )
        .unwrap();
        let cli_path = temp_dir.path().join("scripts/bin/harness-cli");
        fs::write(
            &cli_path,
            "#!/bin/sh\nprintf '%s\n' \"$@\" >> sync-args.log\necho '{\"id\":\"run_local_sync\",\"applied\":true,\"operations\":2}'\n",
        )
        .unwrap();
        make_executable(&cli_path);

        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_local_sync".to_owned(),
                story_id: "US-LOCAL-SYNC".to_owned(),
                branch: Some("symphony/run_local_sync".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_local_sync/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "review local run artifacts".to_owned(),
            })
            .unwrap();

        let response = handle_request(
            &config,
            "POST /api/runs/run_local_sync/sync HTTP/1.1\r\n\r\n",
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""id":"run_local_sync""#));
    }

    #[test]
    fn board_shows_completed_local_run_as_review_when_pr_creation_is_disabled() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = test_config(&temp_dir);
        config.pull_request_create = "disabled".to_owned();
        seed_story_with_status(
            &config.harness_db,
            "US-LOCAL-BOARD",
            "Local board review",
            "planned",
        );
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_local_board".to_owned(),
                story_id: "US-LOCAL-BOARD".to_owned(),
                branch: Some("symphony/run_local_board".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_local_board/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action:
                    "pull request creation failed: PR creation is disabled by pull_request.create"
                        .to_owned(),
            })
            .unwrap();
        store.update_pr_status("run_local_board", "failed").unwrap();

        let response = handle_request(&config, "GET /api/board HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""id":"US-LOCAL-BOARD""#));
        assert!(response.contains(r#""board_state":"Review""#));
        assert!(response.contains("review local run artifacts"));
        assert!(!response.contains("PR creation failure"));
    }

    #[test]
    fn review_hides_pr_retry_for_completed_local_run_when_pr_creation_is_disabled() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = test_config(&temp_dir);
        config.pull_request_create = "disabled".to_owned();
        seed_story_with_status(
            &config.harness_db,
            "US-ATTN",
            "Local review without PR",
            "planned",
        );
        add_test_run(&config, "run_local_review_failed_pr", "completed");
        RunStateStore::new(config.state_db.clone())
            .update_pr_status("run_local_review_failed_pr", "failed")
            .unwrap();
        write_summary(&config, "run_local_review_failed_pr");
        write_result(
            &config,
            "run_local_review_failed_pr",
            r#"{"version":1,"run_id":"run_local_review_failed_pr","story_id":"US-ATTN","outcome":"completed","validation":{"commands":[{"command":"cargo test","result":"pass"}]}}"#,
        );

        let response = handle_request(
            &config,
            "GET /api/runs/run_local_review_failed_pr/review HTTP/1.1\r\n\r\n",
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""pr_status":"not_applicable""#));
        assert!(response.contains(r#""failure_summary":null"#));
        assert!(response.contains(r#""recovery_action":null"#));
        assert!(response.contains("Review local run artifacts and approve sync when ready."));
        assert!(!response.contains("Retry PR creation"));
    }

    #[test]
    fn sync_request_requires_merged_pr() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_sync".to_owned(),
                story_id: "US-SYNC".to_owned(),
                branch: Some("symphony/run_sync".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_sync/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "review pull request".to_owned(),
            })
            .unwrap();
        store
            .update_pr_url("run_sync", "https://example.test/pr/1")
            .unwrap();

        let response =
            handle_request(&config, "POST /api/runs/run_sync/sync HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 409 Conflict"));
        assert!(response.contains("pull request must be marked merged"));
    }

    #[test]
    fn sync_request_applies_only_requested_run_changeset() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        fs::create_dir_all(&config.changeset_directory).unwrap();
        fs::create_dir_all(temp_dir.path().join("scripts/bin")).unwrap();
        fs::write(
            config.changeset_directory.join("run_sync.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_sync"}
{"op":"story.update","version":1,"id":"US-SYNC","payload":{"status":"implemented"}}
"#,
        )
        .unwrap();
        fs::write(
            config.changeset_directory.join("run_other.changeset.jsonl"),
            r#"{"op":"changeset.header","version":1,"run_id":"run_other"}
{"op":"story.update","version":1,"id":"US-OTHER","payload":{"status":"implemented"}}
"#,
        )
        .unwrap();
        let cli_path = temp_dir.path().join("scripts/bin/harness-cli");
        fs::write(
            &cli_path,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> sync-args.log\necho '{\"id\":\"run_sync\",\"applied\":true,\"operations\":2}'\n",
        )
        .unwrap();
        make_executable(&cli_path);

        let store = RunStateStore::new(config.state_db.clone());
        store
            .add_run(crate::state::NewRunRecord {
                run_id: "run_sync".to_owned(),
                story_id: "US-SYNC".to_owned(),
                branch: Some("symphony/run_sync".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "completed".to_owned(),
                result_path: Some(PathBuf::from(".harness/runs/run_sync/RESULT.json")),
                sync_status: "not_applied".to_owned(),
                next_action: "review pull request".to_owned(),
            })
            .unwrap();
        store
            .update_pr_url("run_sync", "https://example.test/pr/1")
            .unwrap();
        store.update_pr_status("run_sync", "merged").unwrap();

        let response =
            handle_request(&config, "POST /api/runs/run_sync/sync HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""id":"run_sync""#));
        assert!(!response.contains("run_other"));
        let args = fs::read_to_string(temp_dir.path().join("sync-args.log")).unwrap();
        assert!(args.contains(".harness/changesets/run_sync.changeset.jsonl"));
        assert!(!args.contains("run_other.changeset.jsonl"));
    }

    #[test]
    fn start_request_refuses_when_active_run_exists() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        seed_runnable_story(&config.harness_db, "US-START");
        crate::state::RunStateStore::new(config.state_db.clone())
            .add_run(crate::state::NewRunRecord {
                run_id: "run_active".to_owned(),
                story_id: "US-OTHER".to_owned(),
                branch: Some("symphony/run_active".to_owned()),
                worktree: temp_dir.path().join("worktree"),
                lightweight: false,
                status: "running".to_owned(),
                result_path: None,
                sync_status: "not_applied".to_owned(),
                next_action: "wait".to_owned(),
            })
            .unwrap();

        let response =
            handle_request(&config, "POST /api/tasks/US-START/start HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 409 Conflict"));
        assert!(response.contains("active run already exists"));
    }

    #[test]
    fn start_request_prepares_run_and_returns_active_record() {
        let temp_dir = tempfile::tempdir().unwrap();
        init_git_repo(temp_dir.path());
        let mut config = test_config(&temp_dir);
        config.agent_adapter = "custom".to_owned();
        config.agent_command = vec!["sh".to_owned(), "-c".to_owned(), "sleep 1".to_owned()];
        seed_runnable_story(&config.harness_db, "US-START");

        let response =
            handle_request(&config, "POST /api/tasks/US-START/start HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 202 Accepted"));
        assert!(response.contains(r#""story_id":"US-START""#));
        let active = RunStateStore::new(config.state_db.clone())
            .active_run()
            .unwrap()
            .unwrap();
        assert_eq!(active.story_id, "US-START");
    }

    #[test]
    fn start_request_with_agent_override_runs_and_remembers_default() {
        let temp_dir = tempfile::tempdir().unwrap();
        init_git_repo(temp_dir.path());
        let mut config = test_config(&temp_dir);
        config.agent_adapter = "opencode".to_owned();
        let fake_opencode = temp_dir.path().join("fake-opencode");
        fs::write(&fake_opencode, "#!/usr/bin/env sh\nexit 0\n").unwrap();
        make_executable(&fake_opencode);
        config.agent_command = vec![fake_opencode.to_str().unwrap().to_owned()];
        seed_runnable_story(&config.harness_db, "US-START");

        let response = handle_request(
            &config,
            "POST /api/tasks/US-START/start HTTP/1.1\r\n\r\n{\"agent\":\"opencode\"}",
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 202 Accepted"));
        assert!(response.contains(r#""agent":"opencode""#));
        let store = RunStateStore::new(config.state_db.clone());
        assert_eq!(
            store.get_setting("default_agent").unwrap(),
            Some("opencode".to_owned())
        );
        let active = store.active_run().unwrap().unwrap();
        assert_eq!(active.agent, "opencode");
    }

    #[test]
    fn context_endpoint_returns_harness_context_pack() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        write_fake_harness_cli(&config);

        let response = handle_request(
            &config,
            "GET /api/tasks/US-CONTEXT/context HTTP/1.1\r\n\r\n",
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""story_id":"US-CONTEXT""#));
        assert!(response.contains("Story US-CONTEXT ready."));
    }

    #[test]
    fn traces_endpoint_filters_harness_trace_records() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        seed_trace_table(&config.harness_db);

        let response = handle_request(
            &config,
            "GET /api/traces?story_id=US-TRACE&outcome=completed HTTP/1.1\r\n\r\n",
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""total":1"#));
        assert!(response.contains(r#""summary":"Trace target""#));
        assert!(response.contains(r#""duration_seconds":12"#));
        assert!(response.contains(r#""harness_friction":"manual review needed""#));
        assert!(!response.contains("Trace other"));
    }

    #[test]
    fn tools_endpoint_returns_cli_registry_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        write_fake_harness_cli(&config);

        let response = handle_request(&config, "GET /api/tools HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""name":"query tools""#));
        assert!(response.contains(r#""capability":"tool-access""#));
        assert!(response.contains(r#""source":"compiled""#));
    }

    #[test]
    fn tools_check_endpoint_returns_scan_result() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        write_fake_harness_cli(&config);

        let response = handle_request(&config, "POST /api/tools/check HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(r#""tools":["#));
        assert!(response.contains(r#""name":"query tools""#));
        assert!(response.contains(r#""detail":"ok""#));
    }

    #[test]
    fn removed_reject_endpoint_returns_not_found_and_keeps_run_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        add_test_run(&config, "run_reject", "completed");

        let response = handle_request(
            &config,
            "POST /api/runs/run_reject/reject HTTP/1.1\r\n\r\n{\"reason\":\"Needs tests\"}",
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 404 Not Found"));
        let run = RunStateStore::new(config.state_db.clone())
            .show_run("run_reject")
            .unwrap();
        assert_eq!(run.status, "completed");
    }

    fn spawn_test_server(config: ResolvedConfig) -> std::net::SocketAddr {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || serve(&config, listener));
        address
    }

    fn fetch_health(address: std::net::SocketAddr) -> String {
        use std::io::Read;

        let mut stream = std::net::TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    #[test]
    fn serve_answers_requests_while_another_connection_is_idle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let address = spawn_test_server(config);

        let _idle = std::net::TcpStream::connect(address).unwrap();
        let response = fetch_health(address);

        assert!(response.starts_with("HTTP/1.1 200 OK"));
    }

    #[test]
    fn serve_survives_clients_that_disconnect_early() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let address = spawn_test_server(config);

        drop(std::net::TcpStream::connect(address).unwrap());
        for _ in 0..3 {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream.write_all(b"GET /health HTTP/1.1\r\n\r\n").unwrap();
            drop(stream);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));

        let response = fetch_health(address);
        assert!(response.starts_with("HTTP/1.1 200 OK"));
    }

    struct BrokenPipeStream {
        request: std::io::Cursor<Vec<u8>>,
    }

    impl std::io::Read for BrokenPipeStream {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            std::io::Read::read(&mut self.request, buf)
        }
    }

    impl Write for BrokenPipeStream {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "broken pipe",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "broken pipe",
            ))
        }
    }

    #[test]
    fn connection_handler_swallows_response_write_failures() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let mut stream = BrokenPipeStream {
            request: std::io::Cursor::new(b"GET /health HTTP/1.1\r\n\r\n".to_vec()),
        };

        handle_connection(&config, &mut stream);
    }

    #[test]
    fn root_serves_built_web_ui_index() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let dist = web_dist_dir(&config);
        fs::create_dir_all(&dist).unwrap();
        fs::write(dist.join("index.html"), "<div id=\"root\"></div>").unwrap();

        let response = handle_request(&config, "GET / HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("text/html"));
        assert!(response.ends_with("<div id=\"root\"></div>"));
    }

    #[test]
    fn static_assets_are_served_as_raw_bytes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let dist = web_dist_dir(&config);
        fs::create_dir_all(&dist).unwrap();
        let asset = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0xff, 0x00];
        fs::write(dist.join("icon.png"), asset).unwrap();

        let response = handle_request(&config, "GET /icon.png HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: image/png"));
        assert!(response.contains("Content-Length: 10"));
        assert_eq!(response.body(), asset);
    }

    #[test]
    fn web_dist_dir_supports_packaged_desktop_override() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);
        let override_dir = temp_dir.path().join("packaged-web-ui-dist");

        let dist = web_dist_dir_with_override(&config, Some(override_dir.clone().into_os_string()));

        assert_eq!(dist, override_dir);
    }

    #[test]
    fn root_reports_missing_built_assets() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = test_config(&temp_dir);

        let response = handle_request(&config, "GET / HTTP/1.1\r\n\r\n").unwrap();

        assert!(response.starts_with("HTTP/1.1 503 Service Unavailable"));
        assert!(response.contains("web UI assets are not built"));
    }
}
