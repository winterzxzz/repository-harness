use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChangesetError {
    #[error("changeset io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("changeset parse error at {path} line {line}: {source}")]
    Parse {
        path: String,
        line: usize,
        source: serde_json::Error,
    },
    #[error("changeset {0} does not start with changeset.header")]
    MissingHeader(String),
    #[error("changeset {path} belongs to run {actual}, expected {expected}")]
    HeaderMismatch {
        path: String,
        actual: String,
        expected: String,
    },
    #[error("changeset {0} contains no semantic operations")]
    Empty(String),
    #[error("changeset {path} contains invalid semantic operation at line {line}: {reason}")]
    InvalidOperation {
        path: String,
        line: usize,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedChange {
    pub operation: String,
    pub entity: String,
    pub change: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedChangeset {
    pub id: String,
    pub path: PathBuf,
    pub rows: Vec<RenderedChange>,
}

pub fn changeset_files(directory: &Path) -> Result<Vec<PathBuf>, ChangesetError> {
    let mut paths = Vec::new();
    if !directory.exists() {
        return Ok(paths);
    }
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let is_changeset = path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.ends_with(".changeset.jsonl"));
        if is_changeset {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

pub fn changeset_id(path: &Path) -> Result<String, ChangesetError> {
    let operations = read_operations(path)?;
    let header = operations
        .first()
        .filter(|value| value.get("op").and_then(Value::as_str) == Some("changeset.header"))
        .ok_or_else(|| ChangesetError::MissingHeader(path.display().to_string()))?;
    Ok(header
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned())
}

pub fn validate_run_changeset(path: &Path, expected_run_id: &str) -> Result<usize, ChangesetError> {
    let operations = read_operations(path)?;
    let header = operations
        .first()
        .filter(|value| value.get("op").and_then(Value::as_str) == Some("changeset.header"))
        .ok_or_else(|| ChangesetError::MissingHeader(path.display().to_string()))?;
    let actual = header
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if actual != expected_run_id {
        return Err(ChangesetError::HeaderMismatch {
            path: path.display().to_string(),
            actual: actual.to_owned(),
            expected: expected_run_id.to_owned(),
        });
    }
    for (index, operation) in operations.iter().enumerate().skip(1) {
        validate_semantic_operation(operation).map_err(|reason| {
            ChangesetError::InvalidOperation {
                path: path.display().to_string(),
                line: index + 1,
                reason,
            }
        })?;
    }
    let semantic = operations.len().saturating_sub(1);
    if semantic == 0 {
        return Err(ChangesetError::Empty(path.display().to_string()));
    }
    Ok(semantic)
}

fn validate_semantic_operation(operation: &Value) -> Result<(), String> {
    let op = operation
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string op".to_owned())?;
    if operation.get("version").and_then(Value::as_i64) != Some(1) {
        return Err("version must be 1".to_owned());
    }
    let numeric_id = matches!(
        op,
        "intake.add" | "backlog.add" | "backlog.close" | "intervention.add" | "trace.add"
    );
    let string_id = matches!(
        op,
        "story.add"
            | "story.update"
            | "story.verify"
            | "decision.add"
            | "decision.verify"
            | "tool.register"
            | "tool.check"
            | "tool.remove"
    );
    if !numeric_id && !string_id {
        return Err(format!("unsupported op {op}"));
    }
    if numeric_id && operation.get("id").and_then(Value::as_i64).is_none() {
        return Err("missing integer id".to_owned());
    }
    if string_id
        && operation
            .get("id")
            .and_then(Value::as_str)
            .is_none_or(|value| value.is_empty())
    {
        return Err("missing non-empty string id".to_owned());
    }
    let payload = operation
        .get("payload")
        .and_then(Value::as_object)
        .ok_or_else(|| "missing object payload".to_owned())?;
    let required = match op {
        "intake.add" => &["input_type", "summary", "risk_lane"][..],
        "story.add" => &["title", "risk_lane"][..],
        "story.verify" | "decision.verify" => &["result"][..],
        "decision.add" => &["title", "status"][..],
        "backlog.add" => &["title"][..],
        "trace.add" => &["task_summary"][..],
        "backlog.close" | "tool.check" => &["status"][..],
        "tool.register" => &["command", "description", "responsibility", "kind"][..],
        "intervention.add" => &["type", "description", "source"][..],
        "story.update" => {
            if !payload.keys().any(|key| {
                matches!(
                    key.as_str(),
                    "status"
                        | "evidence"
                        | "unit_proof"
                        | "integration_proof"
                        | "e2e_proof"
                        | "platform_proof"
                        | "verify_command"
                )
            }) {
                return Err("story.update payload contains no supported fields".to_owned());
            }
            &[][..]
        }
        "tool.remove" => &[][..],
        _ => unreachable!(),
    };
    for field in required {
        if payload
            .get(*field)
            .and_then(Value::as_str)
            .is_none_or(|value| value.is_empty())
        {
            return Err(format!("payload missing non-empty string {field}"));
        }
    }
    Ok(())
}

pub fn render_changeset(path: &Path) -> Result<RenderedChangeset, ChangesetError> {
    let operations = read_operations(path)?;
    let header = operations
        .first()
        .filter(|value| value.get("op").and_then(Value::as_str) == Some("changeset.header"))
        .ok_or_else(|| ChangesetError::MissingHeader(path.display().to_string()))?;
    let id = header
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let rows = operations
        .iter()
        .skip(1)
        .map(render_operation)
        .collect::<Vec<_>>();
    Ok(RenderedChangeset {
        id,
        path: path.to_path_buf(),
        rows,
    })
}

pub fn render_markdown(changeset: &RenderedChangeset, display_path: &str) -> String {
    let mut output = String::new();
    output.push_str("\n## Harness Changes\n\n");
    output.push_str(&format!("Changeset: `{display_path}`\n\n"));
    output.push_str("| Operation | Entity | Change |\n");
    output.push_str("| --- | --- | --- |\n");
    if changeset.rows.is_empty() {
        output.push_str("| - | - | No semantic operations recorded. |\n");
    } else {
        for row in &changeset.rows {
            output.push_str(&format!(
                "| {} | {} | {} |\n",
                escape_table_cell(&row.operation),
                escape_table_cell(&row.entity),
                escape_table_cell(&row.change)
            ));
        }
    }
    output
}

pub fn append_rendered_section(
    summary_path: &Path,
    changeset_path: &Path,
    display_path: &str,
) -> Result<(), ChangesetError> {
    let changeset = render_changeset(changeset_path)?;
    let mut summary = fs::read_to_string(summary_path)?;
    let marker = "\n## Harness Changes\n";
    if let Some(index) = summary.find(marker) {
        summary.truncate(index);
        summary.push('\n');
    }
    summary.push_str(&render_markdown(&changeset, display_path));
    fs::write(summary_path, summary)?;
    Ok(())
}

fn read_operations(path: &Path) -> Result<Vec<Value>, ChangesetError> {
    let text = fs::read_to_string(path)?;
    let mut operations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        operations.push(
            serde_json::from_str(line).map_err(|source| ChangesetError::Parse {
                path: path.display().to_string(),
                line: index + 1,
                source,
            })?,
        );
    }
    Ok(operations)
}

fn render_operation(operation: &Value) -> RenderedChange {
    let op = operation
        .get("op")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let payload = operation.get("payload").unwrap_or(&Value::Null);
    match op.as_str() {
        "intake.add" => RenderedChange {
            operation: op,
            entity: format!("#{}", number_or_unknown(operation, "id")),
            change: format!(
                "{} intake in {} lane",
                string_or_unknown(payload, "input_type"),
                string_or_unknown(payload, "risk_lane")
            ),
        },
        "story.add" => RenderedChange {
            operation: op,
            entity: string_or_unknown(operation, "id"),
            change: format!("added story \"{}\"", string_or_unknown(payload, "title")),
        },
        "story.update" => RenderedChange {
            operation: op,
            entity: string_or_unknown(operation, "id"),
            change: summarize_payload(payload),
        },
        "story.verify" => RenderedChange {
            operation: op,
            entity: string_or_unknown(operation, "id"),
            change: format!("verification {}", string_or_unknown(payload, "result")),
        },
        "decision.add" => RenderedChange {
            operation: op,
            entity: string_or_unknown(operation, "id"),
            change: format!("added decision \"{}\"", string_or_unknown(payload, "title")),
        },
        "trace.add" => RenderedChange {
            operation: op,
            entity: payload
                .get("story_id")
                .and_then(Value::as_str)
                .unwrap_or("trace")
                .to_owned(),
            change: format!("outcome {}", string_or_unknown(payload, "outcome")),
        },
        _ => RenderedChange {
            operation: op,
            entity: operation
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("-")
                .to_owned(),
            change: summarize_payload(payload),
        },
    }
}

fn summarize_payload(payload: &Value) -> String {
    let Some(object) = payload.as_object() else {
        return "payload recorded".to_owned();
    };
    let mut parts = object
        .iter()
        .filter_map(|(key, value)| {
            if value.is_null() {
                None
            } else {
                Some(format!("{key}: {}", value_summary(value)))
            }
        })
        .collect::<Vec<_>>();
    parts.sort();
    if parts.is_empty() {
        "payload recorded".to_owned()
    } else {
        parts.join(", ")
    }
}

fn value_summary(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => "structured value".to_owned(),
    }
}

fn string_or_unknown(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned()
}

fn number_or_unknown(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn escape_table_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_known_and_unknown_operations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("run_1.changeset.jsonl");
        fs::write(
            &path,
            r#"{"op":"changeset.header","version":1,"run_id":"run_1","base_schema_version":6}
{"op":"story.update","version":1,"id":"US-040","payload":{"status":"implemented","unit_proof":1}}
{"op":"future.op","version":1,"id":"F-1","payload":{"alpha":"beta"}}
"#,
        )
        .unwrap();

        let rendered = render_changeset(&path).unwrap();
        assert_eq!(rendered.id, "run_1");
        assert_eq!(rendered.rows[0].entity, "US-040");
        assert!(rendered.rows[0].change.contains("status: implemented"));
        assert_eq!(rendered.rows[1].operation, "future.op");

        let markdown = render_markdown(&rendered, ".harness/changesets/run_1.changeset.jsonl");
        assert!(markdown.contains("## Harness Changes"));
        assert!(markdown.contains("| story.update | US-040 |"));
        assert!(markdown.contains("future.op"));
    }

    #[test]
    fn validates_matching_header_and_semantic_operations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("run_1.changeset.jsonl");
        fs::write(
            &path,
            "{\"op\":\"changeset.header\",\"version\":1,\"run_id\":\"run_1\"}\n{\"op\":\"story.update\",\"version\":1,\"id\":\"US-094\",\"payload\":{\"status\":\"implemented\"}}\n",
        )
        .unwrap();
        assert_eq!(validate_run_changeset(&path, "run_1").unwrap(), 1);
        assert!(validate_run_changeset(&path, "run_other").is_err());
    }

    #[test]
    fn rejects_structurally_invalid_semantic_operations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("run_1.changeset.jsonl");
        for invalid in [
            "{}",
            r#"{"op":"changeset.header","version":1,"run_id":"run_1"}"#,
            r#"{"op":"story.update","version":1,"payload":{}}"#,
            r#"{"op":"unknown.operation","version":1,"id":"US-094","payload":{}}"#,
        ] {
            fs::write(
                &path,
                format!(
                    "{{\"op\":\"changeset.header\",\"version\":1,\"run_id\":\"run_1\"}}\n{invalid}\n"
                ),
            )
            .unwrap();
            assert!(
                validate_run_changeset(&path, "run_1").is_err(),
                "accepted invalid operation: {invalid}"
            );
        }
    }

    #[test]
    fn appends_rendered_section_to_summary_deterministically() {
        let temp_dir = tempfile::tempdir().unwrap();
        let summary = temp_dir.path().join("SUMMARY.md");
        let changeset = temp_dir.path().join("run_1.changeset.jsonl");
        fs::write(&summary, "# Summary\n\nDone.\n").unwrap();
        fs::write(
            &changeset,
            r#"{"op":"changeset.header","version":1,"run_id":"run_1","base_schema_version":6}
{"op":"trace.add","version":1,"payload":{"story_id":"US-040","outcome":"completed"}}
"#,
        )
        .unwrap();

        append_rendered_section(
            &summary,
            &changeset,
            ".harness/changesets/run_1.changeset.jsonl",
        )
        .unwrap();
        append_rendered_section(
            &summary,
            &changeset,
            ".harness/changesets/run_1.changeset.jsonl",
        )
        .unwrap();

        let text = fs::read_to_string(summary).unwrap();
        assert_eq!(text.matches("## Harness Changes").count(), 1);
        assert!(text.contains("| trace.add | US-040 | outcome completed |"));
    }
}
