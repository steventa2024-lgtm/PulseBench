use thiserror::Error;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("unsafe path '{path}': {reason}")]
    UnsafePath { path: String, reason: String },
    #[error("command rejected: {0}")]
    CommandRejected(String),
    #[error("runtime not available: {0}")]
    MissingRuntime(String),
    #[error("toolchain setup failed: {0}")]
    Toolchain(String),
    #[error("workspace error: {0}")]
    Workspace(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SandboxError>;
