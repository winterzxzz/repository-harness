use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

const DEFAULT_MAX_EVENTS: usize = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunEvent {
    pub sequence: u64,
    pub timestamp: String,
    pub agent: String,
    pub kind: String,
    pub stage: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EventPage {
    pub events: Vec<RunEvent>,
    pub last_sequence: u64,
    pub reset_required: bool,
}

#[derive(Clone)]
pub struct RunEventWriter {
    inner: Arc<Mutex<WriterState>>,
}

struct WriterState {
    path: PathBuf,
    agent: String,
    next_sequence: u64,
    max_events: usize,
}

impl RunEventWriter {
    pub fn new(path: PathBuf, agent: impl Into<String>) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let page = read_events_after(&path, None)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(WriterState {
                path,
                agent: agent.into(),
                next_sequence: page.last_sequence.saturating_add(1),
                max_events: DEFAULT_MAX_EVENTS,
            })),
        })
    }

    #[cfg(test)]
    fn with_limit(path: PathBuf, agent: &str, max_events: usize) -> std::io::Result<Self> {
        let writer = Self::new(path, agent)?;
        writer.inner.lock().unwrap().max_events = max_events;
        Ok(writer)
    }

    pub fn append(
        &self,
        kind: &str,
        stage: &str,
        message: impl Into<String>,
    ) -> std::io::Result<RunEvent> {
        let mut state = self.inner.lock().expect("run event writer poisoned");
        let event = RunEvent {
            sequence: state.next_sequence,
            timestamp: rfc3339_timestamp()?,
            agent: state.agent.clone(),
            kind: kind.to_owned(),
            stage: stage.to_owned(),
            message: message.into(),
        };
        state.next_sequence = state.next_sequence.saturating_add(1);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&state.path)?;
        serde_json::to_writer(&mut file, &event).map_err(std::io::Error::other)?;
        file.write_all(b"\n")?;
        file.flush()?;
        compact(&state.path, state.max_events)?;
        Ok(event)
    }
}

pub fn read_events_after(path: &Path, after: Option<u64>) -> std::io::Result<EventPage> {
    if !path.exists() {
        return Ok(EventPage {
            events: Vec::new(),
            last_sequence: 0,
            reset_required: false,
        });
    }
    let mut retained_from = 1;
    let mut all = Vec::new();
    for line in BufReader::new(fs::File::open(path)?).lines() {
        let line = line?;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if let Some(dropped) = value.get("dropped_through").and_then(|v| v.as_u64()) {
            retained_from = dropped.saturating_add(1);
        } else if let Ok(event) = serde_json::from_value::<RunEvent>(value) {
            all.push(event);
        }
    }
    let last_sequence = all
        .last()
        .map_or(retained_from.saturating_sub(1), |event| event.sequence);
    let reset_required = after.is_some_and(|cursor| cursor.saturating_add(1) < retained_from);
    let events = all
        .into_iter()
        .filter(|event| after.is_none_or(|cursor| event.sequence > cursor))
        .collect();
    Ok(EventPage {
        events,
        last_sequence,
        reset_required,
    })
}

fn compact(path: &Path, max_events: usize) -> std::io::Result<()> {
    let page = read_events_after(path, None)?;
    if page.events.len() <= max_events {
        return Ok(());
    }
    let keep_from = page.events.len() - max_events;
    let dropped_through = page.events[keep_from - 1].sequence;
    let mut replacement = Vec::new();
    serde_json::to_writer(
        &mut replacement,
        &serde_json::json!({"dropped_through": dropped_through}),
    )
    .map_err(std::io::Error::other)?;
    replacement.push(b'\n');
    for event in &page.events[keep_from..] {
        serde_json::to_writer(&mut replacement, event).map_err(std::io::Error::other)?;
        replacement.push(b'\n');
    }
    fs::write(path, replacement)
}

fn rfc3339_timestamp() -> std::io::Result<String> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_initialization_creates_event_directory() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("nested/run/RUN_EVENTS.jsonl");

        RunEventWriter::new(path, "codex").unwrap();

        assert!(temp.path().join("nested/run").is_dir());
    }

    #[test]
    fn event_timestamp_is_rfc3339() {
        use time::{format_description::well_known::Rfc3339, OffsetDateTime};

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("RUN_EVENTS.jsonl");
        let event = RunEventWriter::new(path, "codex")
            .unwrap()
            .append("message", "agent", "hello")
            .unwrap();

        OffsetDateTime::parse(&event.timestamp, &Rfc3339).unwrap();
    }

    #[test]
    fn writes_ordered_events_and_reads_after_cursor() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("RUN_EVENTS.jsonl");
        let writer = RunEventWriter::new(path.clone(), "opencode").unwrap();
        writer.append("output", "agent", "first").unwrap();
        writer
            .append("lifecycle", "validation", "validating")
            .unwrap();
        let page = read_events_after(&path, Some(1)).unwrap();
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.events[0].sequence, 2);
        assert_eq!(page.last_sequence, 2);
        assert!(!page.reset_required);
    }

    #[test]
    fn compaction_reports_stale_cursor_and_retains_newest_event() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("RUN_EVENTS.jsonl");
        let writer = RunEventWriter::with_limit(path.clone(), "codex", 2).unwrap();
        for message in ["one", "two", "terminal"] {
            writer.append("lifecycle", "agent", message).unwrap();
        }
        let page = read_events_after(&path, Some(0)).unwrap();
        assert!(page.reset_required);
        assert_eq!(page.events.last().unwrap().message, "terminal");
    }
}
