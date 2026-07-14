use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

const DEFAULT_MAX_EVENTS: usize = 2_000;
const COMPACTION_INTERVAL: usize = 100;

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
        drop(file);
        if should_compact(state.next_sequence, state.max_events) {
            compact(&state.path, state.max_events)?;
        }
        Ok(event)
    }
}

fn should_compact(next_sequence: u64, max_events: usize) -> bool {
    let interval = max_events.clamp(1, COMPACTION_INTERVAL) as u64;
    next_sequence > max_events as u64 && next_sequence.is_multiple_of(interval)
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

pub fn read_last_event(path: &Path) -> std::io::Result<Option<RunEvent>> {
    const READ_CHUNK_SIZE: u64 = 8 * 1024;

    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut position = file.metadata()?.len();
    let mut line_suffix = Vec::new();
    while position > 0 {
        let start = position.saturating_sub(READ_CHUNK_SIZE);
        let mut chunk = vec![0; (position - start) as usize];
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut chunk)?;
        chunk.extend_from_slice(&line_suffix);

        let mut lines = chunk.split(|byte| *byte == b'\n');
        let first = lines.next().unwrap_or_default().to_vec();
        let complete_lines = lines.collect::<Vec<_>>();
        for line in complete_lines.into_iter().rev() {
            if line.is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_slice::<RunEvent>(line) {
                return Ok(Some(event));
            }
        }
        line_suffix = first;
        position = start;
    }
    if line_suffix.is_empty() {
        return Ok(None);
    }
    Ok(serde_json::from_slice::<RunEvent>(&line_suffix).ok())
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
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&replacement)?;
    temporary.as_file_mut().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
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
    fn compaction_is_batched_after_limit() {
        assert!(!should_compact(2_001, 2_000));
        assert!(should_compact(2_100, 2_000));
        assert!(should_compact(4, 2));
    }

    #[cfg(unix)]
    #[test]
    fn compaction_atomically_replaces_event_file() {
        use std::os::unix::fs::MetadataExt;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("RUN_EVENTS.jsonl");
        let writer = RunEventWriter::with_limit(path.clone(), "codex", 2).unwrap();
        writer.append("message", "agent", "one").unwrap();
        writer.append("message", "agent", "two").unwrap();
        let original_inode = fs::metadata(&path).unwrap().ino();

        writer.append("message", "agent", "three").unwrap();

        assert_ne!(fs::metadata(path).unwrap().ino(), original_inode);
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

    #[test]
    fn last_event_handles_missing_blank_and_malformed_tail_lines() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("RUN_EVENTS.jsonl");
        assert_eq!(read_last_event(&path).unwrap(), None);

        let writer = RunEventWriter::new(path.clone(), "codex").unwrap();
        writer.append("progress", "agent", "first").unwrap();
        let expected = writer.append("progress", "agent", "second").unwrap();
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"\nnot-json\n\n").unwrap();

        assert_eq!(read_last_event(&path).unwrap(), Some(expected));
    }
}
