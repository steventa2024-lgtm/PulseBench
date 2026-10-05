use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NotFound(String),
    #[error("a benchmark is already running ({0})")]
    RunInProgress(String),
    #[error("storage: {0}")]
    Storage(#[from] pulsebench_storage::StorageError),
    #[error("suite: {0}")]
    Core(#[from] pulsebench_core::CoreError),
    #[error("provider: {0}")]
    Provider(#[from] pulsebench_providers::ProviderError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AppError>;
