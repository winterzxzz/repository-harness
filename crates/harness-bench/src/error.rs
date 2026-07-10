/// Errors surfaced by the benchmark scoring engine.
#[derive(Debug, thiserror::Error)]
pub enum BenchError {
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse task spec: {0}")]
    TaskParse(String),

    #[error("failed to parse artifact meta: {0}")]
    MetaParse(String),

    #[error("artifact missing required entry: {0}")]
    ArtifactMissing(String),

    #[error("sqlite error: {0}")]
    Sqlite(String),

    #[error("check '{0}' misconfigured: {1}")]
    CheckConfig(String, String),

    #[error("artifact is for task '{artifact}', but spec is for '{spec}'")]
    TaskMismatch { spec: String, artifact: String },
}
