use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("cannot connect to {url}: {detail}")]
    Unreachable { url: String, detail: String },
    #[error("provider returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("model '{0}' was not found on the provider (is it installed?)")]
    ModelNotFound(String),
    #[error("not enough memory to load the model: {0}")]
    InsufficientMemory(String),
    #[error("generation timed out after {0} s")]
    Timeout(u32),
    #[error("generation cancelled")]
    Cancelled,
    #[error("unexpected provider response: {0}")]
    InvalidResponse(String),
    #[error("connection lost during generation: {0}")]
    Stream(String),
}

impl ProviderError {
    pub fn is_unreachable(&self) -> bool {
        matches!(self, ProviderError::Unreachable { .. })
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, ProviderError::Cancelled)
    }

    pub fn is_timeout(&self) -> bool {
        matches!(self, ProviderError::Timeout(_))
    }
}
