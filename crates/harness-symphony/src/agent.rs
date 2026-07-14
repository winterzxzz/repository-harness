use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use thiserror::Error;

use crate::config::ResolvedConfig;
use crate::run::PreparedRun;
use crate::run_events::RunEventWriter;
use crate::state::{process_start_identity, RunStateStore};

#[cfg(not(test))]
const CODEX_IDLE_RECONCILE_SECONDS: u64 = 30;
#[cfg(test)]
const CODEX_IDLE_RECONCILE_SECONDS: u64 = 1;
const AGENT_OUTPUT_MAX_BYTES: usize = 1024 * 1024;
const OUTPUT_TRUNCATION_MARKER: &str = "\n[output truncated by Harness Symphony]\n";

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("agent.command is not configured. Set agent.command in .harness/symphony.yml.")]
    MissingCommand,
    #[error("unsupported agent adapter '{0}'. Supported adapters: custom, codex, opencode")]
    UnsupportedAdapter(String),
    #[error("agent command failed with status {status}: {stderr}")]
    CommandFailed { status: String, stderr: String },
    #[error("agent exceeded wall-clock timeout of {timeout_minutes} minute(s)")]
    Timeout { timeout_minutes: u32 },
    #[error("codex app-server failed: {0}")]
    Codex(String),
    #[error("agent io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("agent json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("run cancelled by operator")]
    Cancelled,
    #[error("runtime state error: {0}")]
    State(String),
}

struct AgentRuntime {
    store: RunStateStore,
    run_id: String,
    events: RunEventWriter,
    last_heartbeat: Instant,
}

impl AgentRuntime {
    fn start(
        config: &ResolvedConfig,
        prepared: &PreparedRun,
        child_pid: u32,
    ) -> Result<Option<Self>, AgentError> {
        let store = RunStateStore::new(config.state_db.clone());
        if store.show_run(&prepared.run_id).is_err() {
            return Ok(None);
        }
        let identity = process_start_identity(child_pid).unwrap_or_else(|| "unverified".to_owned());
        store
            .begin_execution(
                &prepared.run_id,
                std::process::id(),
                child_pid,
                &identity,
                unix_timestamp(),
            )
            .map_err(|error| AgentError::State(error.to_string()))?;
        let events = RunEventWriter::new(run_events_path(prepared), &config.agent_adapter)?;
        events.append("lifecycle", "agent", "agent process started")?;
        Ok(Some(Self {
            store,
            run_id: prepared.run_id.clone(),
            events,
            last_heartbeat: Instant::now(),
        }))
    }

    fn tick(&mut self) -> Result<(), AgentError> {
        if self
            .store
            .cancellation_requested(&self.run_id)
            .map_err(|error| AgentError::State(error.to_string()))?
        {
            self.events
                .append("warning", "agent", "cancellation requested")?;
            return Err(AgentError::Cancelled);
        }
        if self.last_heartbeat.elapsed() >= Duration::from_secs(1) {
            self.store
                .refresh_heartbeat(&self.run_id, unix_timestamp())
                .map_err(|error| AgentError::State(error.to_string()))?;
            self.last_heartbeat = Instant::now();
        }
        Ok(())
    }
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn run_events_path(prepared: &PreparedRun) -> std::path::PathBuf {
    prepared
        .contract_path
        .parent()
        .unwrap_or(&prepared.worktree)
        .join("RUN_EVENTS.jsonl")
}

pub fn run_agent(config: &ResolvedConfig, prepared: &PreparedRun) -> Result<(), AgentError> {
    match config.agent_adapter.as_str() {
        "custom" => run_custom_agent(config, prepared),
        "codex" => run_codex_agent(config, prepared),
        "opencode" => run_opencode_agent(config, prepared),
        other => Err(AgentError::UnsupportedAdapter(other.to_owned())),
    }
}

pub fn resolved_agent_command(config: &ResolvedConfig) -> Vec<String> {
    if !config.agent_command.is_empty() {
        return config.agent_command.clone();
    }
    if config.agent_adapter == "codex" {
        return vec!["codex".to_owned(), "app-server".to_owned()];
    }
    if config.agent_adapter == "opencode" {
        return vec!["opencode".to_owned(), "run".to_owned(), "--auto".to_owned()];
    }
    Vec::new()
}

pub fn agent_adapter_status(config: &ResolvedConfig) -> Result<String, AgentError> {
    match config.agent_adapter.as_str() {
        "custom" => {
            let command = resolved_agent_command(config);
            if command.is_empty() {
                Err(AgentError::MissingCommand)
            } else {
                Ok(format!("custom command: {}", command.join(" ")))
            }
        }
        "codex" => Ok(format!(
            "codex app-server command: {}; runtime: uncapped (protocol stall guarded)",
            resolved_agent_command(config).join(" ")
        )),
        "opencode" => Ok(format!(
            "opencode headless command: {}; runtime: {} minute(s)",
            resolved_agent_command(config).join(" "),
            config.agent_timeout_minutes
        )),
        other => Err(AgentError::UnsupportedAdapter(other.to_owned())),
    }
}

fn run_custom_agent(config: &ResolvedConfig, prepared: &PreparedRun) -> Result<(), AgentError> {
    run_custom_agent_with_timeout(config, prepared, agent_timeout(config))
}

fn run_custom_agent_with_timeout(
    config: &ResolvedConfig,
    prepared: &PreparedRun,
    timeout: Duration,
) -> Result<(), AgentError> {
    run_custom_agent_with_limits(config, prepared, timeout, AGENT_OUTPUT_MAX_BYTES)
}

fn run_custom_agent_with_limits(
    config: &ResolvedConfig,
    prepared: &PreparedRun,
    timeout: Duration,
    output_limit: usize,
) -> Result<(), AgentError> {
    let command = resolved_agent_command(config);
    if command.is_empty() {
        return Err(AgentError::MissingCommand);
    }
    let output_log_path = agent_output_path(prepared);
    let (status, stderr) = run_streaming_command_controlled(
        base_command(&command, prepared),
        &output_log_path,
        timeout,
        output_limit,
        config.agent_timeout_minutes,
        config,
        prepared,
    )?;
    if status.success() {
        return Ok(());
    }
    Err(AgentError::CommandFailed {
        status: status.to_string(),
        stderr,
    })
}

fn run_opencode_agent(config: &ResolvedConfig, prepared: &PreparedRun) -> Result<(), AgentError> {
    let mut command = resolved_agent_command(config);
    if command.is_empty() {
        return Err(AgentError::MissingCommand);
    }
    command.push(agent_prompt(config, prepared));
    let output_log_path = agent_output_path(prepared);
    let (status, stderr) = run_streaming_command_controlled(
        base_command(&command, prepared),
        &output_log_path,
        agent_timeout(config),
        AGENT_OUTPUT_MAX_BYTES,
        config.agent_timeout_minutes,
        config,
        prepared,
    )?;
    if status.success() {
        return Ok(());
    }
    Err(AgentError::CommandFailed {
        status: status.to_string(),
        stderr,
    })
}

fn run_codex_agent(config: &ResolvedConfig, prepared: &PreparedRun) -> Result<(), AgentError> {
    run_codex_agent_with_timeout(config, prepared, agent_timeout(config))
}

fn run_codex_agent_with_timeout(
    config: &ResolvedConfig,
    prepared: &PreparedRun,
    _timeout: Duration,
) -> Result<(), AgentError> {
    let command = resolved_agent_command(config);
    if command.is_empty() {
        return Err(AgentError::MissingCommand);
    }

    let mut process = base_command(&command, prepared);
    configure_process_group(&mut process);
    let mut child = ProcessTreeGuard::new(
        process
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let mut runtime = AgentRuntime::start(config, prepared, child.id())?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AgentError::Codex("failed to open app-server stdin".to_owned()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AgentError::Codex("failed to open app-server stdout".to_owned()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AgentError::Codex("failed to open app-server stderr".to_owned()))?;

    let stderr_text = Arc::new(Mutex::new(Vec::new()));
    let stderr_log_path = agent_output_path(prepared);
    if let Some(parent) = stderr_log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let stderr_log = Arc::new(Mutex::new(CappedWriter::new(
        &stderr_log_path,
        AGENT_OUTPUT_MAX_BYTES,
    )?));
    let stderr_capture = Arc::clone(&stderr_text);
    let stderr_writer = Arc::clone(&stderr_log);
    std::thread::spawn(move || {
        let mut reader = stderr;
        let mut buffer = [0_u8; 8192];
        let mut captured_len = 0;
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 {
                break;
            }
            if captured_len < AGENT_OUTPUT_MAX_BYTES {
                let mut captured = stderr_capture.lock().expect("stderr capture poisoned");
                let remaining = AGENT_OUTPUT_MAX_BYTES.saturating_sub(captured.len());
                captured.extend_from_slice(&buffer[..count.min(remaining)]);
                captured_len = captured.len();
            }
            let _ = stderr_writer
                .lock()
                .expect("stderr log poisoned")
                .write_chunk(&buffer[..count]);
        }
        let _ = stderr_writer.lock().expect("stderr log poisoned").finish();
    });

    let (line_tx, line_rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if line_tx.send(line).is_err() {
                break;
            }
        }
    });

    terminate_on_error(
        send(
            &mut stdin,
            json!({
                "method": "initialize",
                "id": 0,
                "params": {
                    "clientInfo": {
                        "name": "harness_symphony",
                        "title": "Harness Symphony",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "capabilities": {
                        "experimentalApi": true,
                        "requestAttestation": false
                    }
                }
            }),
        ),
        &mut child,
    )?;

    let event_log_path = prepared
        .contract_path
        .parent()
        .unwrap_or(&prepared.worktree)
        .join("APP_SERVER_EVENTS.jsonl");
    let mut thread_id: Option<String> = None;
    let mut turn_id: Option<String> = None;
    let mut turn_started = false;
    let mut last_event_at = Instant::now();
    let mut last_observed_method = "none".to_owned();
    let mut event_count: u64 = 0;
    let mut next_request_id: i64 = 3;
    let mut pending_state_query: Option<i64> = None;
    loop {
        if let Some(runtime) = runtime.as_mut() {
            if let Err(error) = runtime.tick() {
                terminate_child(&mut child);
                return Err(error);
            }
        }
        let line = match line_rx.recv_timeout(Duration::from_millis(250)) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(status) = child.try_wait()? {
                    let stderr = captured_stderr(&stderr_text);
                    return Err(AgentError::CommandFailed {
                        status: status.to_string(),
                        stderr,
                    });
                }
                if pending_state_query.is_some()
                    && last_event_at.elapsed() >= Duration::from_secs(CODEX_IDLE_RECONCILE_SECONDS)
                {
                    terminate_child(&mut child);
                    return Err(AgentError::Codex(format!(
                        "no app-server events or turn-state response for {} second(s) after reconciliation request. Last app-server method: {last_observed_method}; events: {event_count}; see {}",
                        CODEX_IDLE_RECONCILE_SECONDS,
                        event_log_path.display()
                    )));
                }
                if let (Some(thread_id), Some(_turn_id)) = (&thread_id, &turn_id) {
                    if pending_state_query.is_none()
                        && turn_started
                        && last_event_at.elapsed()
                            >= Duration::from_secs(CODEX_IDLE_RECONCILE_SECONDS)
                    {
                        let request_id = next_request_id;
                        next_request_id += 1;
                        terminate_on_error(
                            send_turn_state_query(&mut stdin, request_id, thread_id),
                            &mut child,
                        )?;
                        pending_state_query = Some(request_id);
                        last_event_at = Instant::now();
                    }
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = child.wait()?;
                let stderr = captured_stderr(&stderr_text);
                return Err(AgentError::CommandFailed {
                    status: status.to_string(),
                    stderr,
                });
            }
        };

        terminate_on_error(append_event_log(&event_log_path, &line), &mut child)?;
        if let Some(runtime) = runtime.as_ref() {
            let kind = if line.contains("error") {
                "error"
            } else {
                "message"
            };
            terminate_on_error(
                runtime
                    .events
                    .append(kind, "agent", line.clone())
                    .map(|_| ())
                    .map_err(AgentError::from),
                &mut child,
            )?;
        }
        let message: Value = terminate_on_error(
            serde_json::from_str(&line).map_err(AgentError::from),
            &mut child,
        )?;
        event_count += 1;
        last_event_at = Instant::now();
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            last_observed_method = method.to_owned();
        } else if let Some(id) = message.get("id").and_then(Value::as_i64) {
            last_observed_method = format!("response:{id}");
        }
        if let Some(error) = message.get("error") {
            if pending_state_query == message.get("id").and_then(Value::as_i64) {
                terminate_child(&mut child);
                return Err(AgentError::Codex(format!(
                    "turn-state query failed: {error}. Last app-server method: {last_observed_method}; events: {event_count}; see {}",
                    event_log_path.display()
                )));
            }
            terminate_child(&mut child);
            return Err(AgentError::Codex(error.to_string()));
        }

        if message.get("id").is_some() && message.get("method").is_some() {
            let method = message
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            terminate_child(&mut child);
            return Err(AgentError::Codex(format!(
                "unsupported app-server request '{method}'. See {}",
                event_log_path.display()
            )));
        }

        let response_id = message.get("id").and_then(Value::as_i64);
        match response_id {
            Some(0) => {
                terminate_on_error(
                    send(&mut stdin, json!({ "method": "initialized", "params": {} })),
                    &mut child,
                )?;
                terminate_on_error(
                    send(
                        &mut stdin,
                        json!({
                            "method": "thread/start",
                            "id": 1,
                            "params": {
                                "cwd": prepared.worktree,
                                "runtimeWorkspaceRoots": [prepared.worktree],
                                "approvalPolicy": "never",
                                "sandbox": "danger-full-access"
                            }
                        }),
                    ),
                    &mut child,
                )?;
            }
            Some(1) => {
                let id = message
                    .pointer("/result/thread/id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        AgentError::Codex("thread/start response missing thread id".to_owned())
                    })?
                    .to_owned();
                thread_id = Some(id.clone());
                terminate_on_error(
                    send_turn_start(&mut stdin, config, &id, prepared),
                    &mut child,
                )?;
            }
            Some(2) => {
                turn_id = message
                    .pointer("/result/turn/id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
            }
            _ => {}
        }

        if pending_state_query == response_id {
            pending_state_query = None;
            if let Some(turn_id) = &turn_id {
                match turn_status_from_query(&message, turn_id) {
                    Some("completed") => {
                        terminate_child(&mut child);
                        return Ok(());
                    }
                    Some("failed") | Some("interrupted") => {
                        let detail = turn_error_from_query(&message, turn_id)
                            .unwrap_or("turn did not complete successfully");
                        terminate_child(&mut child);
                        return Err(AgentError::Codex(format!(
                            "turn status was {} from state query: {detail}",
                            turn_status_from_query(&message, turn_id).unwrap_or("unknown")
                        )));
                    }
                    Some("inProgress") => {
                        last_observed_method = "turn-state:inProgress".to_owned();
                    }
                    Some(other) => {
                        terminate_child(&mut child);
                        return Err(AgentError::Codex(format!(
                            "turn-state query returned unknown status '{other}'. See {}",
                            event_log_path.display()
                        )));
                    }
                    None => {
                        last_observed_method = format!("turn-state:notFound:{turn_id}");
                    }
                }
            }
        }

        if message.get("method").and_then(Value::as_str) == Some("turn/started") {
            turn_started = true;
        }

        if message.get("method").and_then(Value::as_str) == Some("turn/completed") {
            let status = message
                .pointer("/params/turn/status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            terminate_child(&mut child);
            if status == "completed" {
                return Ok(());
            }
            let detail = message
                .pointer("/params/turn/error/message")
                .and_then(Value::as_str)
                .unwrap_or("turn did not complete successfully");
            return Err(AgentError::Codex(format!(
                "turn status was {status}: {detail}"
            )));
        }
    }
}

fn base_command(command: &[String], prepared: &PreparedRun) -> Command {
    let mut process = Command::new(&command[0]);
    process
        .args(&command[1..])
        .current_dir(&prepared.worktree)
        .env("HARNESS_DB_PATH", &prepared.harness_db_path)
        .env("HARNESS_RUN_ID", &prepared.run_id)
        .env("HARNESS_RUN_MODE", "execute");
    process
}

fn agent_timeout(config: &ResolvedConfig) -> Duration {
    Duration::from_secs(u64::from(config.agent_timeout_minutes) * 60)
}

fn agent_output_path(prepared: &PreparedRun) -> std::path::PathBuf {
    prepared
        .contract_path
        .parent()
        .unwrap_or(&prepared.worktree)
        .join("AGENT_OUTPUT.log")
}

fn run_streaming_command_controlled(
    mut command: Command,
    output_path: &Path,
    timeout: Duration,
    output_limit: usize,
    timeout_minutes: u32,
    config: &ResolvedConfig,
    prepared: &PreparedRun,
) -> Result<(ExitStatus, String), AgentError> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    configure_process_group(&mut command);
    let mut child = ProcessTreeGuard::new(
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let mut runtime = AgentRuntime::start(config, prepared, child.id())?;
    let stdout = child.stdout.take().expect("piped stdout missing");
    let stderr = child.stderr.take().expect("piped stderr missing");
    let writer = Arc::new(Mutex::new(CappedWriter::new(output_path, output_limit)?));
    let stderr_text = Arc::new(Mutex::new(Vec::new()));
    let event_writer = runtime.as_ref().map(|runtime| runtime.events.clone());
    let mut stdout_thread = Some(spawn_output_drain_with_events(
        stdout,
        Arc::clone(&writer),
        None,
        event_writer.clone(),
    ));
    let mut stderr_thread = Some(spawn_output_drain_with_events(
        stderr,
        Arc::clone(&writer),
        Some(Arc::clone(&stderr_text)),
        event_writer,
    ));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if let Some(error) = finished_output_error(&mut stdout_thread)
            .or_else(|| finished_output_error(&mut stderr_thread))
        {
            child.terminate();
            join_output_drain(stdout_thread.take());
            join_output_drain(stderr_thread.take());
            return Err(error);
        }
        if let Some(runtime) = runtime.as_mut() {
            if let Err(error) = runtime.tick() {
                child.terminate();
                join_output_drain(stdout_thread.take());
                join_output_drain(stderr_thread.take());
                return Err(error);
            }
        }
        if Instant::now() >= deadline {
            child.terminate();
            join_output_drain(stdout_thread.take());
            join_output_drain(stderr_thread.take());
            return Err(AgentError::Timeout { timeout_minutes });
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout_error = join_output_drain(stdout_thread.take());
    let stderr_error = join_output_drain(stderr_thread.take());
    if let Some(error) = stdout_error.or(stderr_error) {
        return Err(error);
    }
    writer.lock().expect("output writer poisoned").finish()?;
    Ok((status, captured_stderr(&stderr_text)))
}

#[cfg(test)]
fn run_streaming_command_with_writer<F>(
    mut command: Command,
    output_path: &Path,
    timeout: Duration,
    output_limit: usize,
    timeout_minutes: u32,
    create_writer: F,
) -> Result<(ExitStatus, String), AgentError>
where
    F: FnOnce(&Path, usize) -> Result<CappedWriter, std::io::Error>,
{
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    configure_process_group(&mut command);
    let mut child = ProcessTreeGuard::new(
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let stdout = child.stdout.take().expect("piped stdout missing");
    let stderr = child.stderr.take().expect("piped stderr missing");
    let writer = Arc::new(Mutex::new(create_writer(output_path, output_limit)?));
    let stderr_text = Arc::new(Mutex::new(Vec::new()));

    let stdout_thread = spawn_output_drain(stdout, Arc::clone(&writer), None);
    let stderr_thread =
        spawn_output_drain(stderr, Arc::clone(&writer), Some(Arc::clone(&stderr_text)));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            child.terminate();
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            writer.lock().expect("output writer poisoned").finish()?;
            return Err(AgentError::Timeout { timeout_minutes });
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    writer.lock().expect("output writer poisoned").finish()?;
    let stderr = captured_stderr(&stderr_text);
    Ok((status, stderr))
}

#[cfg(test)]
fn spawn_output_drain<R: Read + Send + 'static>(
    mut reader: R,
    writer: Arc<Mutex<CappedWriter>>,
    capture: Option<Arc<Mutex<Vec<u8>>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        let mut captured_len = 0;
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 {
                break;
            }
            if let Some(capture) = &capture {
                if captured_len < AGENT_OUTPUT_MAX_BYTES {
                    let mut captured = capture.lock().expect("stderr capture poisoned");
                    let remaining = AGENT_OUTPUT_MAX_BYTES.saturating_sub(captured.len());
                    captured.extend_from_slice(&buffer[..count.min(remaining)]);
                    captured_len = captured.len();
                }
            }
            let _ = writer
                .lock()
                .expect("output writer poisoned")
                .write_chunk(&buffer[..count]);
        }
    })
}

fn spawn_output_drain_with_events<R: Read + Send + 'static>(
    mut reader: R,
    writer: Arc<Mutex<CappedWriter>>,
    capture: Option<Arc<Mutex<Vec<u8>>>>,
    events: Option<RunEventWriter>,
) -> std::thread::JoinHandle<Result<(), AgentError>> {
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        let mut captured_len = 0;
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            if let Some(capture) = &capture {
                if captured_len < AGENT_OUTPUT_MAX_BYTES {
                    let mut captured = capture.lock().expect("stderr capture poisoned");
                    let remaining = AGENT_OUTPUT_MAX_BYTES.saturating_sub(captured.len());
                    captured.extend_from_slice(&buffer[..count.min(remaining)]);
                    captured_len = captured.len();
                }
            }
            writer
                .lock()
                .expect("output writer poisoned")
                .write_chunk(&buffer[..count])?;
            if let Some(events) = &events {
                let message = String::from_utf8_lossy(&buffer[..count]).trim().to_owned();
                if !message.is_empty() {
                    events.append("output", "agent", message)?;
                }
            }
        }
        Ok(())
    })
}

fn finished_output_error(
    thread: &mut Option<std::thread::JoinHandle<Result<(), AgentError>>>,
) -> Option<AgentError> {
    if !thread.as_ref().is_some_and(|thread| thread.is_finished()) {
        return None;
    }
    join_output_drain(thread.take())
}

fn join_output_drain(
    thread: Option<std::thread::JoinHandle<Result<(), AgentError>>>,
) -> Option<AgentError> {
    match thread?.join() {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some(AgentError::State("agent output drain panicked".to_owned())),
    }
}

struct CappedWriter {
    file: std::fs::File,
    limit: usize,
    remaining: usize,
    truncated: bool,
}

impl CappedWriter {
    fn new(path: &Path, limit: usize) -> Result<Self, std::io::Error> {
        Ok(Self {
            file: OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(path)?,
            limit,
            remaining: limit,
            truncated: false,
        })
    }

    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), std::io::Error> {
        if self.truncated {
            return Ok(());
        }
        let count = bytes.len().min(self.remaining);
        self.file.write_all(&bytes[..count])?;
        self.remaining -= count;
        if count < bytes.len() {
            self.mark_truncated()?;
        }
        Ok(())
    }

    fn mark_truncated(&mut self) -> Result<(), std::io::Error> {
        let marker = OUTPUT_TRUNCATION_MARKER.as_bytes();
        let content_len = self.limit.saturating_sub(marker.len());
        self.file.set_len(content_len as u64)?;
        self.file.seek(SeekFrom::End(0))?;
        self.file
            .write_all(&marker[..marker.len().min(self.limit)])?;
        self.remaining = 0;
        self.truncated = true;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), std::io::Error> {
        self.file.flush()
    }
}

fn captured_stderr(stderr: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&stderr.lock().expect("stderr capture poisoned"))
        .trim()
        .to_owned()
}

struct ProcessTreeGuard {
    child: Child,
    armed: bool,
}

impl ProcessTreeGuard {
    fn new(child: Child) -> Self {
        Self { child, armed: true }
    }

    fn terminate(&mut self) {
        if self.armed {
            terminate_process_tree(&mut self.child);
            self.armed = false;
        }
    }
}

impl std::ops::Deref for ProcessTreeGuard {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.child
    }
}

impl std::ops::DerefMut for ProcessTreeGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.child
    }
}

impl Drop for ProcessTreeGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn terminate_on_error<T>(
    result: Result<T, AgentError>,
    child: &mut ProcessTreeGuard,
) -> Result<T, AgentError> {
    if result.is_err() {
        child.terminate();
    }
    result
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_process_tree(child: &mut Child) {
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    const SIGTERM: i32 = 15;
    const SIGKILL: i32 = 9;
    let process_group = -(child.id() as i32);
    unsafe {
        kill(process_group, SIGTERM);
    }
    for _ in 0..10 {
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    unsafe {
        kill(process_group, SIGKILL);
    }
    let _ = child.wait();
}

#[cfg(not(unix))]
fn terminate_process_tree(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn send(stdin: &mut impl Write, message: Value) -> Result<(), AgentError> {
    writeln!(stdin, "{message}")?;
    stdin.flush()?;
    Ok(())
}

fn append_event_log(path: &Path, line: &str) -> Result<(), AgentError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    let current_len = file.metadata()?.len() as usize;
    let bytes = format!("{line}\n").into_bytes();
    if current_len.saturating_add(bytes.len()) <= AGENT_OUTPUT_MAX_BYTES {
        file.seek(SeekFrom::End(0))?;
        file.write_all(&bytes)?;
    } else {
        let marker = OUTPUT_TRUNCATION_MARKER.as_bytes();
        let content_len = AGENT_OUTPUT_MAX_BYTES.saturating_sub(marker.len());
        if current_len < content_len {
            file.seek(SeekFrom::End(0))?;
            let fill = content_len - current_len;
            file.write_all(&bytes[..fill.min(bytes.len())])?;
        } else {
            file.set_len(content_len as u64)?;
        }
        file.seek(SeekFrom::End(0))?;
        file.write_all(&marker[..marker.len().min(AGENT_OUTPUT_MAX_BYTES)])?;
    }
    Ok(())
}

fn send_turn_start(
    stdin: &mut impl Write,
    config: &ResolvedConfig,
    thread_id: &str,
    prepared: &PreparedRun,
) -> Result<(), AgentError> {
    send(
        stdin,
        json!({
            "method": "turn/start",
            "id": 2,
            "params": {
                "threadId": thread_id,
                "cwd": prepared.worktree,
                "runtimeWorkspaceRoots": [prepared.worktree],
                "approvalPolicy": "never",
                "sandboxPolicy": { "type": "dangerFullAccess" },
                "input": [
                    {
                        "type": "text",
                        "text": agent_prompt(config, prepared),
                        "text_elements": []
                    }
                ]
            }
        }),
    )
}

fn send_turn_state_query(
    stdin: &mut impl Write,
    request_id: i64,
    thread_id: &str,
) -> Result<(), AgentError> {
    send(
        stdin,
        json!({
            "method": "thread/turns/list",
            "id": request_id,
            "params": {
                "threadId": thread_id,
                "limit": 10,
                "sortDirection": "desc",
                "itemsView": "notLoaded"
            }
        }),
    )
}

fn turn_status_from_query<'a>(message: &'a Value, turn_id: &str) -> Option<&'a str> {
    message
        .pointer("/result/data")
        .and_then(Value::as_array)?
        .iter()
        .find(|turn| turn.get("id").and_then(Value::as_str) == Some(turn_id))?
        .get("status")
        .and_then(Value::as_str)
}

fn turn_error_from_query<'a>(message: &'a Value, turn_id: &str) -> Option<&'a str> {
    message
        .pointer("/result/data")
        .and_then(Value::as_array)?
        .iter()
        .find(|turn| turn.get("id").and_then(Value::as_str) == Some(turn_id))?
        .pointer("/error/message")
        .and_then(Value::as_str)
}

fn agent_prompt(config: &ResolvedConfig, prepared: &PreparedRun) -> String {
    let harness_cli = config.repo_root.join("scripts/bin/harness-cli");
    let mut prompt = format!(
        "You are running inside a Harness Symphony worktree. Read AGENTS.md and the run contract at {}. Complete only story {} for run {}. Do not change unrelated product code. Write all required artifacts under the current working directory: .harness/runs/{}/SUMMARY.md and .harness/runs/{}/RESULT.json. Use Harness CLI writes with HARNESS_DB_PATH, HARNESS_RUN_ID, and HARNESS_RUN_MODE from the environment so .harness/changesets/{}.changeset.jsonl is produced in this worktree. If scripts/bin/harness-cli is absent in the worktree, run the root binary at {} while keeping the current worktree as cwd. RESULT.json must have version 1, run_id {}, story_id {}, an allowed outcome, summary_path .harness/runs/{}/SUMMARY.md, and a top-level validation object. Do not write validation_evidence. validation must be either {{\"commands\":[{{\"command\":\"exact command\",\"result\":\"pass\"}}]}} with each result set to pass, fail, or unavailable, or {{\"unavailable\":\"non-empty reason\"}}.",
        prepared.contract_path.display(),
        prepared.story_id,
        prepared.run_id,
        prepared.run_id,
        prepared.run_id,
        prepared.run_id,
        harness_cli.display(),
        prepared.run_id,
        prepared.story_id,
        prepared.run_id
    );
    if let Some(feedback) = prepared.request_changes.as_ref() {
        let evidence_paths = if feedback.evidence_paths.is_empty() {
            "none".to_owned()
        } else {
            feedback.evidence_paths.join(", ")
        };
        prompt.push_str(&format!(
            " This is a request-changes replacement for source run {}. Read the request-changes reason at {} before editing. Inspect every evidence image before editing: {}. If this agent adapter cannot inspect images, report the limitation in SUMMARY.md instead of silently ignoring the evidence.",
            feedback.source_run_id, feedback.reason_path, evidence_paths
        ));
    }
    prompt
}

fn terminate_child(child: &mut ProcessTreeGuard) {
    child.terminate();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedConfig;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn config(adapter: &str, command: Vec<&str>) -> ResolvedConfig {
        ResolvedConfig {
            version: 1,
            repo_root: Path::new("/repo").to_path_buf(),
            harness_db: Path::new("/repo/harness.db").to_path_buf(),
            state_db: Path::new("/repo/.symphony/state.db").to_path_buf(),
            runs_dir: Path::new("/repo/.harness/runs").to_path_buf(),
            worktrees_dir: Path::new("/repo/.symphony/worktrees").to_path_buf(),
            single_active_run: true,
            agent_adapter: adapter.to_owned(),
            agent_command: command.into_iter().map(str::to_owned).collect(),
            agent_timeout_minutes: 120,
            pull_request_create: "ask".to_owned(),
            pull_request_provider: "github".to_owned(),
            pull_request_draft_for: vec![],
            changeset_directory: Path::new("/repo/.harness/changesets").to_path_buf(),
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

    fn prepared() -> PreparedRun {
        PreparedRun {
            run_id: "run_1".to_owned(),
            story_id: "US-046".to_owned(),
            branch: Some("symphony/run_1".to_owned()),
            worktree: Path::new("/repo/.symphony/worktrees/run_1").to_path_buf(),
            contract_path: Path::new("/repo/.harness/runs/run_1/RUN_CONTRACT.json").to_path_buf(),
            harness_db_path: Path::new("/repo/.symphony/worktrees/run_1/harness.db").to_path_buf(),
            lightweight: false,
            request_changes: None,
        }
    }

    #[test]
    fn codex_adapter_defaults_to_app_server_command() {
        let config = config("codex", vec![]);

        assert_eq!(
            resolved_agent_command(&config),
            vec!["codex".to_owned(), "app-server".to_owned()]
        );
        assert!(agent_adapter_status(&config)
            .unwrap()
            .contains("codex app-server"));
    }

    #[test]
    fn custom_adapter_requires_command() {
        let config = config("custom", vec![]);

        assert!(matches!(
            agent_adapter_status(&config).unwrap_err(),
            AgentError::MissingCommand
        ));
    }

    #[test]
    fn agent_prompt_points_to_worktree_artifacts_and_run_env() {
        let config = config("codex", vec![]);
        let prompt = agent_prompt(&config, &prepared());

        assert!(prompt.contains("US-046"));
        assert!(prompt.contains(".harness/runs/run_1/SUMMARY.md"));
        assert!(prompt.contains(".harness/changesets/run_1.changeset.jsonl"));
        assert!(prompt.contains("/repo/scripts/bin/harness-cli"));
        assert!(prompt.contains("HARNESS_DB_PATH"));
        assert!(prompt.contains("top-level validation object"));
        assert!(prompt.contains("Do not write validation_evidence"));
        assert!(prompt.contains("\"result\":\"pass\""));
    }

    #[test]
    fn request_changes_prompt_requires_image_inspection() {
        let config = config("codex", vec![]);
        let mut prepared = prepared();
        prepared.request_changes = Some(crate::run::RequestChangesContract {
            source_run_id: "run_old".to_owned(),
            reason_path: ".harness/runs/run_1/feedback/reason.md".to_owned(),
            evidence_paths: vec![".harness/runs/run_1/feedback/evidence-01.png".to_owned()],
        });

        let prompt = agent_prompt(&config, &prepared);

        assert!(prompt.contains("Read the request-changes reason"));
        assert!(prompt.contains("Inspect every evidence image"));
        assert!(prompt.contains("feedback/reason.md"));
        assert!(prompt.contains("feedback/evidence-01.png"));
        assert!(prompt.contains("report the limitation in SUMMARY.md"));
    }

    #[test]
    fn opencode_adapter_defaults_to_headless_run_command() {
        let config = config("opencode", vec![]);

        assert_eq!(
            resolved_agent_command(&config),
            vec!["opencode".to_owned(), "run".to_owned(), "--auto".to_owned()]
        );
        assert!(agent_adapter_status(&config)
            .unwrap()
            .contains("opencode headless"));
    }

    #[test]
    fn opencode_adapter_passes_prompt_and_logs_output() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let fake_opencode = temp_dir.path().join("fake-opencode");
        fs::write(
            &fake_opencode,
            r#"#!/usr/bin/env sh
for last in "$@"; do :; done
printf '%s\n' "prompt: $last"
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_opencode).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_opencode, permissions).unwrap();

        let config = config("opencode", vec![fake_opencode.to_str().unwrap()]);
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = worktree.join(".harness/runs/run_1/RUN_CONTRACT.json");

        run_opencode_agent(&config, &prepared).unwrap();

        let log =
            fs::read_to_string(worktree.join(".harness/runs/run_1/AGENT_OUTPUT.log")).unwrap();
        assert!(log.contains("prompt: You are running inside a Harness Symphony worktree"));
    }

    #[test]
    fn opencode_adapter_reports_command_failure_with_stderr() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let fake_opencode = temp_dir.path().join("fake-opencode");
        fs::write(
            &fake_opencode,
            r#"#!/usr/bin/env sh
echo "opencode exploded" >&2
exit 3
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_opencode).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_opencode, permissions).unwrap();

        let config = config("opencode", vec![fake_opencode.to_str().unwrap()]);
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = worktree.join(".harness/runs/run_1/RUN_CONTRACT.json");

        let error = run_opencode_agent(&config, &prepared).unwrap_err();
        assert!(matches!(
            error,
            AgentError::CommandFailed { ref stderr, .. } if stderr.contains("opencode exploded")
        ));
    }

    #[test]
    fn custom_adapter_times_out_at_wall_clock_deadline() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let sleeper = temp_dir.path().join("sleeper");
        fs::write(&sleeper, "#!/usr/bin/env sh\nsleep 30\n").unwrap();
        let mut permissions = fs::metadata(&sleeper).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&sleeper, permissions).unwrap();

        let config = config("custom", vec![sleeper.to_str().unwrap()]);
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = worktree.join(".harness/runs/run_1/RUN_CONTRACT.json");

        let started = Instant::now();
        let error = run_custom_agent_with_timeout(&config, &prepared, Duration::from_millis(100))
            .unwrap_err();
        assert!(matches!(error, AgentError::Timeout { .. }));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn custom_adapter_caps_streamed_output_artifact() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let noisy = temp_dir.path().join("noisy");
        fs::write(&noisy, "#!/usr/bin/env sh\nyes x | head -c 20000\n").unwrap();
        let mut permissions = fs::metadata(&noisy).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&noisy, permissions).unwrap();

        let config = config("custom", vec![noisy.to_str().unwrap()]);
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = worktree.join(".harness/runs/run_1/RUN_CONTRACT.json");

        run_custom_agent_with_limits(&config, &prepared, Duration::from_secs(5), 1024).unwrap();

        let log =
            fs::read_to_string(worktree.join(".harness/runs/run_1/AGENT_OUTPUT.log")).unwrap();
        assert!(log.len() <= 1024);
        assert!(log.ends_with(OUTPUT_TRUNCATION_MARKER));
    }

    #[test]
    fn normalized_event_write_failure_is_reported_by_output_drain() {
        let temp_dir = tempfile::tempdir().unwrap();
        let blocked_parent = temp_dir.path().join("not-a-directory");
        let event_writer =
            RunEventWriter::new(blocked_parent.join("RUN_EVENTS.jsonl"), "opencode").unwrap();
        fs::remove_dir(&blocked_parent).unwrap();
        fs::write(&blocked_parent, "file").unwrap();
        let output_path = temp_dir.path().join("AGENT_OUTPUT.log");
        let writer = Arc::new(Mutex::new(
            CappedWriter::new(&output_path, AGENT_OUTPUT_MAX_BYTES).unwrap(),
        ));

        let result = spawn_output_drain_with_events(
            std::io::Cursor::new(b"visible output\n".to_vec()),
            writer,
            None,
            Some(event_writer),
        )
        .join()
        .unwrap();

        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_kills_controlled_adapter_descendants() {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let pid_path = temp_dir.path().join("grandchild.pid");
        let fixture = temp_dir.path().join("cancellable-tree");
        fs::write(
            &fixture,
            r#"#!/usr/bin/env sh
trap 'exit 0' TERM
sh -c 'trap "" TERM; sleep 30' &
echo $! > "$1"
wait
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fixture).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fixture, permissions).unwrap();

        let mut config = config(
            "custom",
            vec![fixture.to_str().unwrap(), pid_path.to_str().unwrap()],
        );
        config.repo_root = temp_dir.path().to_path_buf();
        config.state_db = temp_dir.path().join(".symphony/state.db");
        config.runs_dir = temp_dir.path().join(".harness/runs");
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = config.runs_dir.join("run_1/RUN_CONTRACT.json");

        RunStateStore::new(config.state_db.clone())
            .add_run(crate::state::NewRunRecord {
                run_id: "run_1".to_owned(),
                story_id: "US-046".to_owned(),
                branch: None,
                worktree: worktree.clone(),
                lightweight: false,
                status: "prepared".to_owned(),
                result_path: None,
                sync_status: "not_applied".to_owned(),
                next_action: "run agent".to_owned(),
            })
            .unwrap();

        let state_db = config.state_db.clone();
        let cancel_pid_path = pid_path.clone();
        let canceller = std::thread::spawn(move || {
            let store = RunStateStore::new(state_db);
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if cancel_pid_path.exists()
                    && store
                        .show_run("run_1")
                        .is_ok_and(|run| run.status == "running")
                {
                    store.request_cancel("run_1").unwrap();
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("adapter never entered running state");
        });

        let error =
            run_custom_agent_with_timeout(&config, &prepared, Duration::from_secs(10)).unwrap_err();
        canceller.join().unwrap();
        assert!(matches!(error, AgentError::Cancelled));
        let pid: i32 = fs::read_to_string(pid_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let exists = unsafe { kill(pid, 0) == 0 };
        if exists {
            unsafe {
                kill(pid, 9);
            }
        }
        assert!(!exists, "grandchild {pid} survived cancellation");
    }

    #[cfg(unix)]
    #[test]
    fn streaming_setup_failure_kills_spawned_process_tree() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let ready_path = temp_dir.path().join("descendant-ready");
        let survived_path = temp_dir.path().join("descendant-survived");
        let fixture = temp_dir.path().join("setup-failure-tree");
        fs::write(
            &fixture,
            r#"#!/usr/bin/env sh
sh -c 'touch "$1"; sleep 1; touch "$2"; sleep 30' sh "$1" "$2" &
wait
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fixture).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fixture, permissions).unwrap();
        let output_path = temp_dir.path().join("AGENT_OUTPUT.log");

        let mut command = Command::new(&fixture);
        command
            .arg(&ready_path)
            .arg(&survived_path)
            .current_dir(&worktree);
        let error = run_streaming_command_with_writer(
            command,
            &output_path,
            Duration::from_secs(5),
            1024,
            1,
            |_, _| {
                let deadline = Instant::now() + Duration::from_secs(5);
                while !ready_path.exists() && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
                assert!(ready_path.exists(), "descendant did not become ready");
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "injected writer setup failure",
                ))
            },
        )
        .unwrap_err();

        assert!(matches!(error, AgentError::Io(_)));
        std::thread::sleep(Duration::from_millis(1200));
        assert!(
            !survived_path.exists(),
            "descendant survived post-spawn writer setup failure"
        );
    }

    #[test]
    fn app_server_event_log_includes_marker_within_cap() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("APP_SERVER_EVENTS.jsonl");
        let oversized = "x".repeat(AGENT_OUTPUT_MAX_BYTES + 1024);

        append_event_log(&path, &oversized).unwrap();

        let log = fs::read(&path).unwrap();
        assert!(log.len() <= AGENT_OUTPUT_MAX_BYTES);
        assert!(log.ends_with(OUTPUT_TRUNCATION_MARKER.as_bytes()));
    }

    #[test]
    fn app_server_event_log_replaces_near_cap_tail_with_marker() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("APP_SERVER_EVENTS.jsonl");
        fs::write(&path, vec![b'x'; AGENT_OUTPUT_MAX_BYTES - 5]).unwrap();

        append_event_log(&path, "more than five bytes").unwrap();

        let log = fs::read(&path).unwrap();
        assert!(log.len() <= AGENT_OUTPUT_MAX_BYTES);
        assert!(log.ends_with(OUTPUT_TRUNCATION_MARKER.as_bytes()));
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_descendants_after_direct_child_exits() {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let fixture = temp_dir.path().join("process-tree");
        fs::write(
            &fixture,
            r#"#!/usr/bin/env sh
trap 'exit 0' TERM
sh -c 'trap "" TERM; sleep 30' &
echo $! > "$1"
wait
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fixture).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fixture, permissions).unwrap();

        let pid_path = temp_dir.path().join("grandchild.pid");
        let mut command = Command::new(&fixture);
        command.arg(&pid_path).current_dir(&worktree);
        configure_process_group(&mut command);
        let mut child = command.spawn().unwrap();
        let ready_deadline = Instant::now() + Duration::from_secs(5);
        while !pid_path.exists() && Instant::now() < ready_deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let pid: i32 = fs::read_to_string(pid_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        terminate_process_tree(&mut child);
        std::thread::sleep(Duration::from_millis(50));
        let exists = unsafe { kill(pid, 0) == 0 };
        if exists {
            unsafe {
                kill(pid, 9);
            }
        }
        assert!(!exists, "grandchild {pid} survived timeout cleanup");
    }

    #[test]
    fn codex_adapter_continuously_drains_stderr() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let fake_server = temp_dir.path().join("fake-codex-app-server");
        fs::write(
            &fake_server,
            r#"#!/usr/bin/env sh
read initialize
printf '%s\n' '{"id":0,"result":{}}'
read initialized
read thread_start
printf '%s\n' '{"id":1,"result":{"thread":{"id":"thr_1"}}}'
read turn_start
dd if=/dev/zero bs=1024 count=1024 1>&2 2>/dev/null
printf '%s\n' '{"id":2,"result":{}}'
printf '%s\n' '{"method":"turn/completed","params":{"turn":{"status":"completed"}}}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_server).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_server, permissions).unwrap();

        let config = config("codex", vec![fake_server.to_str().unwrap()]);
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = worktree.join(".harness/runs/run_1/RUN_CONTRACT.json");

        run_codex_agent_with_timeout(&config, &prepared, Duration::from_secs(5)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn codex_adapter_observes_cancellation_while_events_are_continuous() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let gate = temp_dir.path().join("cancel-requested");
        let ready = temp_dir.path().join("event-stream-ready");
        let fake_server = temp_dir.path().join("busy-codex-app-server");
        fs::write(
            &fake_server,
            r#"#!/usr/bin/env sh
read initialize
printf '%s\n' '{"id":0,"result":{}}'
read initialized
read thread_start
printf '%s\n' '{"id":1,"result":{"thread":{"id":"thr_1"}}}'
read turn_start
printf '%s\n' '{"id":2,"result":{"turn":{"id":"turn_1"}}}'
printf '%s\n' '{"method":"turn/started","params":{"turn":{"id":"turn_1"}}}'
touch "$2"
while [ ! -f "$1" ]; do
  printf '%s\n' '{"method":"item/agentMessage/delta","params":{"delta":"busy"}}'
done
i=0
while [ "$i" -lt 1000 ]; do
  printf '%s\n' '{"method":"item/agentMessage/delta","params":{"delta":"busy"}}'
  i=$((i + 1))
done
printf '%s\n' '{"method":"turn/completed","params":{"turn":{"status":"completed"}}}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_server).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_server, permissions).unwrap();

        let mut config = config(
            "codex",
            vec![
                fake_server.to_str().unwrap(),
                gate.to_str().unwrap(),
                ready.to_str().unwrap(),
            ],
        );
        config.repo_root = temp_dir.path().to_path_buf();
        config.state_db = temp_dir.path().join(".symphony/state.db");
        config.runs_dir = temp_dir.path().join(".harness/runs");
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = config.runs_dir.join("run_1/RUN_CONTRACT.json");

        RunStateStore::new(config.state_db.clone())
            .add_run(crate::state::NewRunRecord {
                run_id: "run_1".to_owned(),
                story_id: "US-046".to_owned(),
                branch: None,
                worktree,
                lightweight: false,
                status: "prepared".to_owned(),
                result_path: None,
                sync_status: "not_applied".to_owned(),
                next_action: "run agent".to_owned(),
            })
            .unwrap();

        let state_db = config.state_db.clone();
        let cancel_gate = gate.clone();
        let stream_ready = ready.clone();
        let canceller = std::thread::spawn(move || {
            let store = RunStateStore::new(state_db);
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if stream_ready.exists()
                    && store
                        .show_run("run_1")
                        .is_ok_and(|run| run.status == "running")
                {
                    store.request_cancel("run_1").unwrap();
                    fs::write(cancel_gate, "requested").unwrap();
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("Codex adapter never entered running state");
        });

        let error =
            run_codex_agent_with_timeout(&config, &prepared, Duration::from_secs(5)).unwrap_err();
        canceller.join().unwrap();
        assert!(matches!(error, AgentError::Cancelled));
    }

    #[cfg(unix)]
    #[test]
    fn codex_post_spawn_validation_error_kills_process_tree() {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let fake_server = temp_dir.path().join("malformed-codex-app-server");
        fs::write(
            &fake_server,
            r#"#!/usr/bin/env sh
read initialize
printf '%s\n' '{"id":0,"result":{}}'
read initialized
read thread_start
sh -c 'trap "" TERM; sleep 30' &
echo $! > "$1"
printf '%s\n' '{"id":1,"result":{}}'
wait
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_server).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_server, permissions).unwrap();

        let pid_path = temp_dir.path().join("descendant.pid");
        let config = config(
            "codex",
            vec![fake_server.to_str().unwrap(), pid_path.to_str().unwrap()],
        );
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = worktree.join(".harness/runs/run_1/RUN_CONTRACT.json");

        let error =
            run_codex_agent_with_timeout(&config, &prepared, Duration::from_secs(5)).unwrap_err();
        assert!(matches!(error, AgentError::Codex(_)));
        let pid: i32 = fs::read_to_string(pid_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let exists = unsafe { kill(pid, 0) == 0 };
        if exists {
            unsafe {
                kill(pid, 9);
            }
        }
        assert!(!exists, "descendant {pid} survived protocol error cleanup");
    }

    #[test]
    fn codex_adapter_completes_json_rpc_handshake() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let fake_server = temp_dir.path().join("fake-codex-app-server");
        fs::write(
            &fake_server,
            r#"#!/usr/bin/env sh
read initialize
printf '%s\n' '{"id":0,"result":{"userAgent":"fake","codexHome":"/tmp","platformFamily":"unix","platformOs":"macos"}}'
read initialized
read thread_start
printf '%s\n' '{"id":1,"result":{"thread":{"id":"thr_1"}}}'
printf '%s\n' '{"method":"thread/started","params":{"thread":{"id":"thr_1"}}}'
read turn_start
printf '%s\n' '{"id":2,"result":{}}'
printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr_1","turn":{"id":"turn_1","items":[],"itemsView":{"type":"complete"},"status":"completed","error":null,"startedAt":1,"completedAt":2,"durationMs":1000}}}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_server).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_server, permissions).unwrap();

        let mut config = config("codex", vec![fake_server.to_str().unwrap()]);
        config.agent_timeout_minutes = 1;
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = worktree.join(".harness/runs/run_1/RUN_CONTRACT.json");

        run_codex_agent(&config, &prepared).unwrap();
    }

    #[test]
    fn codex_adapter_reconciles_before_wall_clock_deadline() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let fake_server = temp_dir.path().join("fake-codex-app-server");
        fs::write(
            &fake_server,
            r#"#!/usr/bin/env sh
read initialize
printf '%s\n' '{"id":0,"result":{"userAgent":"fake","codexHome":"/tmp","platformFamily":"unix","platformOs":"macos"}}'
read initialized
read thread_start
printf '%s\n' '{"id":1,"result":{"thread":{"id":"thr_1"}}}'
read turn_start
printf '%s\n' '{"id":2,"result":{"turn":{"id":"turn_1","items":[],"itemsView":"notLoaded","status":"inProgress","error":null,"startedAt":null,"completedAt":null,"durationMs":null}}}'
printf '%s\n' '{"method":"turn/started","params":{"threadId":"thr_1","turn":{"id":"turn_1","items":[],"itemsView":"notLoaded","status":"inProgress","error":null,"startedAt":1,"completedAt":null,"durationMs":null}}}'
read state_query_one
printf '%s\n' '{"id":3,"result":{"data":[{"id":"turn_1","items":[],"itemsView":"notLoaded","status":"inProgress","error":null,"startedAt":1,"completedAt":null,"durationMs":null}],"nextCursor":null,"backwardsCursor":null}}'
read state_query_two
printf '%s\n' '{"id":4,"result":{"data":[{"id":"turn_1","items":[],"itemsView":"notLoaded","status":"completed","error":null,"startedAt":1,"completedAt":2,"durationMs":1000}],"nextCursor":null,"backwardsCursor":null}}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_server).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_server, permissions).unwrap();

        let mut config = config("codex", vec![fake_server.to_str().unwrap()]);
        config.agent_timeout_minutes = 1;
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = worktree.join(".harness/runs/run_1/RUN_CONTRACT.json");

        run_codex_agent(&config, &prepared).unwrap();
    }

    #[test]
    fn codex_adapter_reports_failed_terminal_turn() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let fake_server = temp_dir.path().join("fake-codex-app-server");
        fs::write(
            &fake_server,
            r#"#!/usr/bin/env sh
read initialize
printf '%s\n' '{"id":0,"result":{"userAgent":"fake","codexHome":"/tmp","platformFamily":"unix","platformOs":"macos"}}'
read initialized
read thread_start
printf '%s\n' '{"id":1,"result":{"thread":{"id":"thr_1"}}}'
read turn_start
printf '%s\n' '{"id":2,"result":{}}'
printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr_1","turn":{"id":"turn_1","items":[],"itemsView":{"type":"complete"},"status":"failed","error":{"message":"boom"},"startedAt":1,"completedAt":2,"durationMs":1000}}}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_server).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_server, permissions).unwrap();

        let config = config("codex", vec![fake_server.to_str().unwrap()]);
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = worktree.join(".harness/runs/run_1/RUN_CONTRACT.json");

        let error = run_codex_agent(&config, &prepared).unwrap_err();
        assert!(error.to_string().contains("turn status was failed: boom"));
    }

    #[test]
    fn codex_adapter_recovers_completed_turn_from_state_query() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worktree = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        let fake_server = temp_dir.path().join("fake-codex-app-server");
        fs::write(
            &fake_server,
            r#"#!/usr/bin/env sh
read initialize
printf '%s\n' '{"id":0,"result":{"userAgent":"fake","codexHome":"/tmp","platformFamily":"unix","platformOs":"macos"}}'
read initialized
read thread_start
printf '%s\n' '{"id":1,"result":{"thread":{"id":"thr_1"}}}'
read turn_start
printf '%s\n' '{"id":2,"result":{"turn":{"id":"turn_1","items":[],"itemsView":"notLoaded","status":"inProgress","error":null,"startedAt":null,"completedAt":null,"durationMs":null}}}'
printf '%s\n' '{"method":"turn/started","params":{"threadId":"thr_1","turn":{"id":"turn_1","items":[],"itemsView":"notLoaded","status":"inProgress","error":null,"startedAt":1,"completedAt":null,"durationMs":null}}}'
read state_query
printf '%s\n' '{"id":3,"result":{"data":[{"id":"turn_1","items":[],"itemsView":"notLoaded","status":"completed","error":null,"startedAt":1,"completedAt":2,"durationMs":1000}],"nextCursor":null,"backwardsCursor":null}}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_server).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_server, permissions).unwrap();

        let mut config = config("codex", vec![fake_server.to_str().unwrap()]);
        config.agent_timeout_minutes = 1;
        let mut prepared = prepared();
        prepared.worktree = worktree.clone();
        prepared.harness_db_path = worktree.join("harness.db");
        prepared.contract_path = worktree.join(".harness/runs/run_1/RUN_CONTRACT.json");

        run_codex_agent(&config, &prepared).unwrap();
    }
}
