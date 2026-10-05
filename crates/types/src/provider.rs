//! Types exchanged with inference providers.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::config::{GenerationSettings, ProviderKind};

/// Metadata about an installed model. Everything the provider did not report is `None`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelInfo {
    pub provider_id: String,
    pub provider_kind: ProviderKind,
    /// Identifier to pass back to the provider (`qwen3-coder:30b`).
    pub id: String,
    pub display_name: String,
    pub family: Option<String>,
    pub architecture: Option<String>,
    pub parameter_size: Option<String>,
    pub quantization: Option<String>,
    #[ts(type = "number | null")]
    pub size_bytes: Option<u64>,
    #[ts(type = "number | null")]
    pub context_length: Option<u64>,
    pub format: Option<String>,
    /// Currently loaded into memory (if the provider reports it).
    pub loaded: Option<bool>,
    pub modified_at: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ConnectionState {
    Connected,
    Unreachable,
    Error,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProviderStatus {
    pub provider_id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub state: ConnectionState,
    pub version: Option<String>,
    pub error: Option<String>,
    pub latency_ms: Option<u32>,
    pub models: Vec<ModelInfo>,
}

/// What a provider can and cannot honour; recorded with every result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProviderCapabilities {
    pub honors_seed: bool,
    /// `false` means the provider uses whatever context the model was loaded with.
    pub honors_context_length: bool,
    pub reports_token_counts: bool,
    pub reports_timing: bool,
}

/// Provider description stored with results (no credentials).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProviderSnapshot {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub version: Option<String>,
    pub capabilities: ProviderCapabilities,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GenerationRequest {
    pub model: String,
    pub system: String,
    pub prompt: String,
    pub settings: GenerationSettings,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum FinishReason {
    /// Model stopped on its own.
    Stop,
    /// Output hit the token limit (truncated).
    Length,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum MetricSource {
    /// Counts/timings reported by the provider itself.
    ProviderReported,
    /// Derived from wall-clock measurements taken by PulseBench.
    ClientMeasured,
}

/// Result of one generation call. Missing metrics stay `None`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GenerationResult {
    pub text: String,
    /// Reasoning/"thinking" text if the provider returns it separately.
    pub reasoning: Option<String>,
    pub model: String,
    pub provider: String,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub duration_ms: u32,
    pub time_to_first_token_ms: Option<u32>,
    pub tokens_per_second: Option<f64>,
    pub load_duration_ms: Option<u32>,
    pub finish_reason: Option<FinishReason>,
    pub metric_source: Option<MetricSource>,
}

/// Live progress while streaming.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GenerationProgress {
    pub chars: u32,
    /// Streamed chunks received (an approximation of tokens for most servers).
    pub chunks: u32,
    pub elapsed_ms: u32,
    pub time_to_first_token_ms: Option<u32>,
}

/// Memory footprint of a loaded model as reported by the provider (Ollama `/api/ps`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelResidency {
    #[ts(type = "number")]
    pub size_bytes: u64,
    #[ts(type = "number | null")]
    pub size_vram_bytes: Option<u64>,
    pub context_length: Option<u32>,
}
