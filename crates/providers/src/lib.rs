//! Inference provider abstraction.
//!
//! Benchmark logic only ever talks to [`InferenceProvider`]; adding llama.cpp, vLLM or a hosted
//! API later means adding one more implementation here.

mod error;
mod http;
mod ollama;
mod openai;
pub mod testing;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use pulsebench_types::{
    ConnectionState, GenerationProgress, GenerationRequest, GenerationResult, GenerationSettings, ModelInfo, ModelResidency,
    ProviderCapabilities, ProviderConfig, ProviderKind, ProviderSnapshot, ProviderStatus,
};
use tokio_util::sync::CancellationToken;

pub use error::ProviderError;
pub use ollama::OllamaProvider;
pub use openai::OpenAiCompatProvider;

pub type Result<T> = std::result::Result<T, ProviderError>;

/// Callback invoked as streamed output arrives.
pub type ProgressFn<'a> = &'a (dyn Fn(GenerationProgress) + Send + Sync);

/// Information returned by [`InferenceProvider::detect`].
#[derive(Clone, Debug, PartialEq)]
pub struct Detection {
    pub version: Option<String>,
    pub latency_ms: u32,
}

#[async_trait]
pub trait InferenceProvider: Send + Sync {
    fn config(&self) -> &ProviderConfig;
    fn capabilities(&self) -> ProviderCapabilities;

    /// Is a server listening here, and which version is it?
    async fn detect(&self) -> Result<Detection>;
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;
    async fn model_info(&self, model: &str) -> Result<ModelInfo>;

    /// Run one completion. Streams progress through `on_progress`, aborts promptly when `cancel`
    /// fires and when `request.settings.timeout_seconds` elapses.
    async fn generate(
        &self,
        request: &GenerationRequest,
        cancel: &CancellationToken,
        on_progress: ProgressFn<'_>,
    ) -> Result<GenerationResult>;

    /// Cheap liveness check.
    async fn health(&self) -> Result<()> {
        self.detect().await.map(|_| ())
    }

    /// Load the model with the run's settings so the first benchmark task does not pay load time.
    async fn warm_up(&self, _model: &str, _settings: &GenerationSettings) -> Result<()> {
        Ok(())
    }

    /// Release the model from memory so the next model's VRAM measurements are clean.
    async fn unload(&self, _model: &str) -> Result<()> {
        Ok(())
    }

    /// Memory footprint of the loaded model, if the provider reports it.
    async fn residency(&self, _model: &str) -> Result<Option<ModelResidency>> {
        Ok(None)
    }

    fn id(&self) -> &str {
        &self.config().id
    }
}

/// Build the provider implementation for a configuration entry.
pub fn build_provider(config: &ProviderConfig) -> Arc<dyn InferenceProvider> {
    match config.kind {
        ProviderKind::Ollama => Arc::new(OllamaProvider::new(config.clone())),
        ProviderKind::OpenaiCompatible => Arc::new(OpenAiCompatProvider::new(config.clone())),
    }
}

/// Providers keyed by id.
#[derive(Clone, Default)]
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn InferenceProvider>>,
}

impl ProviderRegistry {
    pub fn from_configs(configs: &[ProviderConfig]) -> Self {
        let mut r = Self::default();
        for c in configs.iter().filter(|c| c.enabled) {
            r.insert(build_provider(c));
        }
        r
    }

    pub fn insert(&mut self, provider: Arc<dyn InferenceProvider>) {
        self.providers.insert(provider.id().to_string(), provider);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn InferenceProvider>> {
        self.providers.get(id).cloned()
    }

    pub fn all(&self) -> Vec<Arc<dyn InferenceProvider>> {
        let mut v: Vec<_> = self.providers.values().cloned().collect();
        v.sort_by(|a, b| a.id().cmp(b.id()));
        v
    }
}

/// Detect a provider and list its models without ever failing: problems become part of the
/// returned status so the UI can show them.
pub async fn probe(provider: &dyn InferenceProvider) -> ProviderStatus {
    let cfg = provider.config();
    let mut status = ProviderStatus {
        provider_id: cfg.id.clone(),
        name: cfg.name.clone(),
        kind: cfg.kind,
        base_url: cfg.base_url.clone(),
        state: ConnectionState::Connected,
        version: None,
        error: None,
        latency_ms: None,
        models: vec![],
    };
    let started = Instant::now();
    match provider.detect().await {
        Ok(d) => {
            status.version = d.version;
            status.latency_ms = Some(d.latency_ms);
        }
        Err(e) => {
            status.state = if e.is_unreachable() { ConnectionState::Unreachable } else { ConnectionState::Error };
            status.error = Some(e.to_string());
            status.latency_ms = Some(started.elapsed().as_millis().min(u32::MAX as u128) as u32);
            return status;
        }
    }
    match provider.list_models().await {
        Ok(models) => status.models = models,
        Err(e) => {
            status.state = ConnectionState::Error;
            status.error = Some(format!("Connected, but listing models failed: {e}"));
        }
    }
    status
}

/// A disabled provider placeholder for status lists.
pub fn disabled_status(cfg: &ProviderConfig) -> ProviderStatus {
    ProviderStatus {
        provider_id: cfg.id.clone(),
        name: cfg.name.clone(),
        kind: cfg.kind,
        base_url: cfg.base_url.clone(),
        state: ConnectionState::Disabled,
        version: None,
        error: None,
        latency_ms: None,
        models: vec![],
    }
}

pub async fn snapshot(provider: &dyn InferenceProvider) -> ProviderSnapshot {
    let cfg = provider.config();
    let version = provider.detect().await.ok().and_then(|d| d.version);
    ProviderSnapshot {
        id: cfg.id.clone(),
        name: cfg.name.clone(),
        kind: cfg.kind,
        base_url: cfg.base_url.clone(),
        version,
        capabilities: provider.capabilities(),
    }
}
