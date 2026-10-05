use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("suite error: {0}")]
    Suite(String),
    #[error("invalid task '{task}': {reason}")]
    Task { task: String, reason: String },
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON in {path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub(crate) fn io_err(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> CoreError + '_ {
    move |source| CoreError::Io { path: path.display().to_string(), source }
}
