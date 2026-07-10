use std::path::Path;

use serde::Deserialize;

use crate::error::BenchError;

/// A benchmark task's expected-proof spec, loaded from `expected.toml`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TaskSpec {
    pub id: String,
    pub lane: String,
    pub functional: Functional,
    #[serde(default)]
    pub checks: Vec<Check>,
}

/// The cross-arm functional check: a shell command whose exit status is pass/fail.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Functional {
    pub test_command: String,
}

/// One harness-only check, tagged with the responsibility it measures.
///
/// `kind` selects the check type: `"sql_expect"` compares the first column of
/// `sql` to `expect`; `"sql_nonzero"` passes when the first column of `sql` is
/// a count greater than zero.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Check {
    pub id: String,
    pub responsibility: String,
    pub kind: String,
    #[serde(default)]
    pub sql: Option<String>,
    #[serde(default)]
    pub expect: Option<String>,
}

impl TaskSpec {
    pub fn from_toml_str(text: &str) -> Result<Self, BenchError> {
        toml::from_str(text).map_err(|e| BenchError::TaskParse(e.to_string()))
    }

    pub fn load(path: &Path) -> Result<Self, BenchError> {
        let text = std::fs::read_to_string(path).map_err(|source| BenchError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_toml_str(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
id = "T1"
lane = "tiny"

[functional]
test_command = "cargo test"

[[checks]]
id = "intake_lane"
responsibility = "Task specification"
kind = "sql_expect"
sql = "SELECT lane FROM intake ORDER BY id DESC LIMIT 1"
expect = "tiny"

[[checks]]
id = "trace_recorded"
responsibility = "Observability"
kind = "sql_nonzero"
sql = "SELECT count(*) FROM trace"
"#;

    #[test]
    fn parses_task_spec_fields() {
        let spec = TaskSpec::from_toml_str(SAMPLE).expect("valid spec parses");
        assert_eq!(spec.id, "T1");
        assert_eq!(spec.lane, "tiny");
        assert_eq!(spec.functional.test_command, "cargo test");
        assert_eq!(spec.checks.len(), 2);
        assert_eq!(spec.checks[0].id, "intake_lane");
        assert_eq!(spec.checks[0].responsibility, "Task specification");
        assert_eq!(spec.checks[0].kind, "sql_expect");
        assert_eq!(spec.checks[0].expect.as_deref(), Some("tiny"));
        assert_eq!(spec.checks[1].kind, "sql_nonzero");
    }

    #[test]
    fn rejects_malformed_toml() {
        let err = TaskSpec::from_toml_str("id = ").unwrap_err();
        assert!(matches!(err, BenchError::TaskParse(_)));
    }
}
