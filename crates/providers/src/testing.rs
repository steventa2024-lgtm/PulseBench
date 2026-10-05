//! Scripted provider for tests and CI. It implements the real [`InferenceProvider`] trait but is
//! never registered by the application; it exists only so the benchmark engine can be exercised
//! without a model server.

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use pulsebench_types::{
    FinishReason, GenerationProgress, GenerationRequest, GenerationResult, MetricSource, ModelInfo, ProviderCapabilities, ProviderConfig,
    ProviderKind,
};
use tokio_util::sync::CancellationToken;

use crate::error::ProviderError;
use crate::{Detection, InferenceProvider, ProgressFn, Result};

/// How the scripted provider answers a request.
pub type Responder = Box<dyn Fn(&GenerationRequest, u32) -> std::result::Result<String, ProviderError> + Send + Sync>;

pub struct ScriptedProvider {
    config: ProviderConfig,
    models: Vec<String>,
    responder: Responder,
    calls: Mutex<u32>,
    /// Requests seen, in order (system, prompt).
    pub seen: Mutex<VecDeque<GenerationRequest>>,
    /// Simulated generation delay in milliseconds (useful for cancellation tests).
    pub delay_ms: u64,
}

impl ScriptedProvider {
    pub fn new(id: &str, models: &[&str], responder: Responder) -> Self {
        Self {
            config: ProviderConfig {
                id: id.into(),
                kind: ProviderKind::OpenaiCompatible,
                name: format!("Scripted {id}"),
                base_url: "http://scripted.invalid".into(),
                enabled: true,
                api_key: None,
            },
            models: models.iter().map(|s| s.to_string()).collect(),
            responder,
            calls: Mutex::new(0),
            seen: Mutex::new(VecDeque::new()),
            delay_ms: 0,
        }
    }

    pub fn with_delay(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }

    pub fn call_count(&self) -> u32 {
        *self.calls.lock().unwrap()
    }
}

#[async_trait]
impl InferenceProvider for ScriptedProvider {
    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities { honors_seed: true, honors_context_length: true, reports_token_counts: true, reports_timing: true }
    }

    async fn detect(&self) -> Result<Detection> {
        Ok(Detection { version: Some("scripted".into()), latency_ms: 0 })
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(self
            .models
            .iter()
            .map(|id| ModelInfo {
                provider_id: self.config.id.clone(),
                provider_kind: self.config.kind,
                id: id.clone(),
                display_name: id.clone(),
                family: None,
                architecture: None,
                parameter_size: None,
                quantization: None,
                size_bytes: None,
                context_length: None,
                format: None,
                loaded: None,
                modified_at: None,
            })
            .collect())
    }

    async fn model_info(&self, model: &str) -> Result<ModelInfo> {
        self.list_models().await?.into_iter().find(|m| m.id == model).ok_or_else(|| ProviderError::ModelNotFound(model.into()))
    }

    async fn generate(
        &self,
        request: &GenerationRequest,
        cancel: &CancellationToken,
        on_progress: ProgressFn<'_>,
    ) -> Result<GenerationResult> {
        let n = {
            let mut c = self.calls.lock().unwrap();
            *c += 1;
            *c
        };
        self.seen.lock().unwrap().push_back(request.clone());
        if self.delay_ms > 0 {
            tokio::select! {
                _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
                _ = tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)) => {}
            }
        }
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        let text = (self.responder)(request, n)?;
        let completion = (text.len() / 4).max(1) as u32;
        on_progress(GenerationProgress { chars: text.len() as u32, chunks: completion, elapsed_ms: 10, time_to_first_token_ms: Some(5) });
        Ok(GenerationResult {
            text,
            reasoning: None,
            model: request.model.clone(),
            provider: self.config.id.clone(),
            prompt_tokens: Some((request.prompt.len() / 4) as u32),
            completion_tokens: Some(completion),
            total_tokens: Some((request.prompt.len() / 4) as u32 + completion),
            duration_ms: 1000,
            time_to_first_token_ms: Some(5),
            tokens_per_second: Some(completion as f64),
            load_duration_ms: None,
            finish_reason: Some(FinishReason::Stop),
            metric_source: Some(MetricSource::ClientMeasured),
        })
    }
}
