//! Ollama provider (`/api/tags`, `/api/show`, `/api/chat`, `/api/ps`, `/api/version`).

use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::StreamExt;
use pulsebench_types::{
    FinishReason, GenerationProgress, GenerationRequest, GenerationResult, GenerationSettings, MetricSource, ModelInfo, ModelResidency,
    ProviderCapabilities, ProviderConfig, ProviderKind,
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::error::ProviderError;
use crate::http::{build_client, error_from_response, join, map_reqwest, LineSplitter};
use crate::{Detection, InferenceProvider, ProgressFn, Result};

const KEEP_ALIVE: &str = "30m";

pub struct OllamaProvider {
    config: ProviderConfig,
    client: Client,
}

impl OllamaProvider {
    pub fn new(config: ProviderConfig) -> Self {
        let client = build_client(&config.base_url);
        Self { config, client }
    }

    fn url(&self, path: &str) -> String {
        join(&self.config.base_url, path)
    }

    async fn get_json(&self, path: &str, timeout: Duration) -> Result<Value> {
        let resp = self.client.get(self.url(path)).timeout(timeout).send().await.map_err(|e| map_reqwest(&self.config.base_url, e))?;
        if !resp.status().is_success() {
            return Err(error_from_response("", resp).await);
        }
        resp.json::<Value>().await.map_err(|e| ProviderError::InvalidResponse(e.to_string()))
    }

    async fn show(&self, model: &str) -> Result<Value> {
        let resp = self
            .client
            .post(self.url("/api/show"))
            .timeout(Duration::from_secs(10))
            .json(&json!({ "model": model }))
            .send()
            .await
            .map_err(|e| map_reqwest(&self.config.base_url, e))?;
        if !resp.status().is_success() {
            return Err(error_from_response(model, resp).await);
        }
        resp.json::<Value>().await.map_err(|e| ProviderError::InvalidResponse(e.to_string()))
    }

    async fn loaded_models(&self) -> Vec<String> {
        match self.get_json("/api/ps", Duration::from_secs(5)).await {
            Ok(v) => parse_ps(&v).into_iter().map(|(name, _)| name).collect(),
            Err(_) => vec![],
        }
    }

    async fn chat_control(&self, model: &str, settings: &GenerationSettings, keep_alive: Value) -> Result<()> {
        let body = json!({
            "model": model,
            "messages": [],
            "stream": false,
            "keep_alive": keep_alive,
            "options": { "num_ctx": settings.context_tokens },
        });
        let resp = self
            .client
            .post(self.url("/api/chat"))
            .timeout(Duration::from_secs(settings.timeout_seconds.max(120) as u64))
            .json(&body)
            .send()
            .await
            .map_err(|e| map_reqwest(&self.config.base_url, e))?;
        if !resp.status().is_success() {
            return Err(error_from_response(model, resp).await);
        }
        Ok(())
    }
}

#[async_trait]
impl InferenceProvider for OllamaProvider {
    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities { honors_seed: true, honors_context_length: true, reports_token_counts: true, reports_timing: true }
    }

    async fn detect(&self) -> Result<Detection> {
        let started = Instant::now();
        let v = self.get_json("/api/version", Duration::from_secs(3)).await?;
        let version = v.get("version").and_then(Value::as_str).map(str::to_string);
        Ok(Detection { version, latency_ms: started.elapsed().as_millis() as u32 })
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let tags = self.get_json("/api/tags", Duration::from_secs(10)).await?;
        let mut models = parse_tags(&self.config.id, &tags)?;
        let loaded = self.loaded_models().await;

        // Enrich with /api/show (context length, architecture, capabilities), tolerating failures.
        let ids: Vec<String> = models.iter().map(|m| m.id.clone()).collect();
        let shows = futures_util::stream::iter(ids)
            .map(|id| async move { (id.clone(), self.show(&id).await) })
            .buffered(4)
            .collect::<Vec<_>>()
            .await;
        let mut keep = Vec::with_capacity(models.len());
        for (m, (_, show)) in models.drain(..).zip(shows) {
            let mut m = m;
            m.loaded = Some(loaded.iter().any(|l| l == &m.id));
            if let Ok(show) = show {
                if !apply_show(&mut m, &show) {
                    continue; // embedding-only model
                }
            }
            keep.push(m);
        }
        Ok(keep)
    }

    async fn model_info(&self, model: &str) -> Result<ModelInfo> {
        let all = self.list_models().await?;
        all.into_iter().find(|m| m.id == model).ok_or_else(|| ProviderError::ModelNotFound(model.to_string()))
    }

    async fn generate(
        &self,
        request: &GenerationRequest,
        cancel: &CancellationToken,
        on_progress: ProgressFn<'_>,
    ) -> Result<GenerationResult> {
        let s = &request.settings;
        let mut options = json!({
            "temperature": s.temperature,
            "num_ctx": s.context_tokens,
            "num_predict": s.max_output_tokens,
        });
        if let Some(seed) = s.seed {
            options["seed"] = json!(seed);
        }
        let body = json!({
            "model": request.model,
            "messages": [
                { "role": "system", "content": request.system },
                { "role": "user", "content": request.prompt },
            ],
            "stream": true,
            "keep_alive": KEEP_ALIVE,
            "options": options,
        });

        let started = Instant::now();
        let deadline = Duration::from_secs(s.timeout_seconds as u64);
        let work = async {
            let resp =
                self.client.post(self.url("/api/chat")).json(&body).send().await.map_err(|e| map_reqwest(&self.config.base_url, e))?;
            if !resp.status().is_success() {
                return Err(error_from_response(&request.model, resp).await);
            }

            let mut acc = ChatAccumulator::default();
            let mut splitter = LineSplitter::default();
            let mut stream = resp.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| ProviderError::Stream(e.to_string()))?;
                for line in splitter.push(&chunk) {
                    if acc.ingest_line(&line, started.elapsed())? {
                        on_progress(acc.progress(started.elapsed()));
                    }
                }
            }
            if let Some(line) = splitter.finish() {
                acc.ingest_line(&line, started.elapsed())?;
            }
            if !acc.done {
                return Err(ProviderError::Stream("stream ended before the model finished".into()));
            }
            Ok(acc)
        };

        let acc = tokio::select! {
            _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
            r = tokio::time::timeout(deadline, work) => match r {
                Ok(r) => r?,
                Err(_) => return Err(ProviderError::Timeout(s.timeout_seconds)),
            },
        };
        Ok(acc.into_result(&request.model, &self.config.id, started.elapsed()))
    }

    async fn warm_up(&self, model: &str, settings: &GenerationSettings) -> Result<()> {
        self.chat_control(model, settings, json!(KEEP_ALIVE)).await
    }

    async fn unload(&self, model: &str) -> Result<()> {
        let settings = GenerationSettings::default();
        self.chat_control(model, &settings, json!(0)).await
    }

    async fn residency(&self, model: &str) -> Result<Option<ModelResidency>> {
        let v = self.get_json("/api/ps", Duration::from_secs(5)).await?;
        Ok(parse_ps(&v).into_iter().find(|(n, _)| n == model).map(|(_, r)| r))
    }
}

// ---------------------------------------------------------------------------------------------
// Parsing (kept free of I/O so it is unit-testable)
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    modified_at: Option<String>,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    details: Option<TagDetails>,
}

#[derive(Deserialize, Default)]
struct TagDetails {
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    parameter_size: Option<String>,
    #[serde(default)]
    quantization_level: Option<String>,
}

pub(crate) fn parse_tags(provider_id: &str, v: &Value) -> Result<Vec<ModelInfo>> {
    let parsed: TagsResponse = serde_json::from_value(v.clone()).map_err(|e| ProviderError::InvalidResponse(format!("/api/tags: {e}")))?;
    Ok(parsed
        .models
        .into_iter()
        .map(|m| {
            let d = m.details.unwrap_or_default();
            let id = m.model.filter(|s| !s.is_empty()).unwrap_or_else(|| m.name.clone());
            ModelInfo {
                provider_id: provider_id.to_string(),
                provider_kind: ProviderKind::Ollama,
                display_name: m.name,
                id,
                family: d.family.clone().filter(|s| !s.is_empty()),
                architecture: d.family.filter(|s| !s.is_empty()),
                parameter_size: d.parameter_size.filter(|s| !s.is_empty()),
                quantization: d.quantization_level.filter(|s| !s.is_empty()),
                size_bytes: m.size,
                context_length: None,
                format: d.format.filter(|s| !s.is_empty()),
                loaded: None,
                modified_at: m.modified_at,
            }
        })
        .collect())
}

/// Merge `/api/show` data into a model. Returns `false` for embedding-only models.
pub(crate) fn apply_show(m: &mut ModelInfo, show: &Value) -> bool {
    if let Some(caps) = show.get("capabilities").and_then(Value::as_array) {
        let caps: Vec<&str> = caps.iter().filter_map(Value::as_str).collect();
        if !caps.is_empty() && !caps.contains(&"completion") {
            return false;
        }
    }
    if let Some(info) = show.get("model_info").and_then(Value::as_object) {
        if let Some(arch) = info.get("general.architecture").and_then(Value::as_str) {
            m.architecture = Some(arch.to_string());
            if let Some(ctx) = info.get(&format!("{arch}.context_length")).and_then(Value::as_u64) {
                m.context_length = Some(ctx);
            }
        }
    }
    if let Some(d) = show.get("details") {
        if m.parameter_size.is_none() {
            m.parameter_size = d.get("parameter_size").and_then(Value::as_str).map(str::to_string);
        }
        if m.quantization.is_none() {
            m.quantization = d.get("quantization_level").and_then(Value::as_str).map(str::to_string);
        }
    }
    true
}

pub(crate) fn parse_ps(v: &Value) -> Vec<(String, ModelResidency)> {
    v.get("models")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let name = m.get("model").or_else(|| m.get("name")).and_then(Value::as_str)?.to_string();
                    let size_bytes = m.get("size").and_then(Value::as_u64)?;
                    Some((
                        name,
                        ModelResidency {
                            size_bytes,
                            size_vram_bytes: m.get("size_vram").and_then(Value::as_u64),
                            context_length: m.get("context_length").and_then(Value::as_u64).map(|c| c as u32),
                        },
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Default)]
pub(crate) struct ChatAccumulator {
    text: String,
    reasoning: String,
    chunks: u32,
    first_token: Option<Duration>,
    pub(crate) done: bool,
    done_reason: Option<String>,
    prompt_eval_count: Option<u32>,
    eval_count: Option<u32>,
    eval_duration_ns: Option<u64>,
    load_duration_ns: Option<u64>,
}

impl ChatAccumulator {
    /// Parse one NDJSON line. Returns `true` if it carried new output.
    pub(crate) fn ingest_line(&mut self, line: &str, elapsed: Duration) -> Result<bool> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(false);
        }
        let v: Value = serde_json::from_str(line).map_err(|e| ProviderError::InvalidResponse(format!("bad stream line: {e}")))?;
        if let Some(err) = v.get("error").and_then(Value::as_str) {
            let lower = err.to_lowercase();
            return Err(if lower.contains("memory") {
                ProviderError::InsufficientMemory(err.to_string())
            } else {
                ProviderError::Stream(err.to_string())
            });
        }
        let mut produced = false;
        if let Some(msg) = v.get("message") {
            if let Some(c) = msg.get("content").and_then(Value::as_str) {
                if !c.is_empty() {
                    self.text.push_str(c);
                    produced = true;
                }
            }
            if let Some(t) = msg.get("thinking").and_then(Value::as_str) {
                if !t.is_empty() {
                    self.reasoning.push_str(t);
                    produced = true;
                }
            }
        }
        if produced {
            self.chunks += 1;
            self.first_token.get_or_insert(elapsed);
        }
        if v.get("done").and_then(Value::as_bool).unwrap_or(false) {
            self.done = true;
            self.done_reason = v.get("done_reason").and_then(Value::as_str).map(str::to_string);
            self.prompt_eval_count = v.get("prompt_eval_count").and_then(Value::as_u64).map(|n| n as u32);
            self.eval_count = v.get("eval_count").and_then(Value::as_u64).map(|n| n as u32);
            self.eval_duration_ns = v.get("eval_duration").and_then(Value::as_u64);
            self.load_duration_ns = v.get("load_duration").and_then(Value::as_u64);
        }
        Ok(produced)
    }

    pub(crate) fn progress(&self, elapsed: Duration) -> GenerationProgress {
        GenerationProgress {
            chars: (self.text.chars().count() + self.reasoning.chars().count()) as u32,
            chunks: self.chunks,
            elapsed_ms: elapsed.as_millis() as u32,
            time_to_first_token_ms: self.first_token.map(|d| d.as_millis() as u32),
        }
    }

    pub(crate) fn into_result(self, model: &str, provider: &str, elapsed: Duration) -> GenerationResult {
        let (tps, source) = match (self.eval_count, self.eval_duration_ns) {
            (Some(n), Some(ns)) if ns > 0 && n > 0 => (Some(n as f64 / (ns as f64 / 1e9)), Some(MetricSource::ProviderReported)),
            _ => (None, None),
        };
        let total = match (self.prompt_eval_count, self.eval_count) {
            (Some(p), Some(c)) => Some(p + c),
            _ => None,
        };
        GenerationResult {
            text: self.text,
            reasoning: if self.reasoning.is_empty() { None } else { Some(self.reasoning) },
            model: model.to_string(),
            provider: provider.to_string(),
            prompt_tokens: self.prompt_eval_count,
            completion_tokens: self.eval_count,
            total_tokens: total,
            duration_ms: elapsed.as_millis() as u32,
            time_to_first_token_ms: self.first_token.map(|d| d.as_millis() as u32),
            tokens_per_second: tps,
            load_duration_ms: self.load_duration_ns.map(|ns| (ns / 1_000_000) as u32),
            finish_reason: self.done_reason.as_deref().map(|r| match r {
                "stop" => FinishReason::Stop,
                "length" => FinishReason::Length,
                _ => FinishReason::Other,
            }),
            metric_source: source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tags_and_show() {
        let tags = json!({"models":[{
            "name":"qwen3-coder:30b","model":"qwen3-coder:30b","modified_at":"2025-09-01T10:00:00Z","size":18556701934u64,
            "details":{"format":"gguf","family":"qwen3moe","parameter_size":"30.5B","quantization_level":"Q4_K_M"}
        },{"name":"nomic-embed-text:latest","model":"nomic-embed-text:latest","size":274302450u64,"details":{}}]});
        let mut models = parse_tags("ollama", &tags).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].quantization.as_deref(), Some("Q4_K_M"));
        assert_eq!(models[0].parameter_size.as_deref(), Some("30.5B"));
        assert_eq!(models[0].size_bytes, Some(18556701934));

        let show = json!({"capabilities":["completion","tools"],"model_info":{"general.architecture":"qwen3moe","qwen3moe.context_length":262144}});
        assert!(apply_show(&mut models[0], &show));
        assert_eq!(models[0].context_length, Some(262144));
        assert_eq!(models[0].architecture.as_deref(), Some("qwen3moe"));

        let embed = json!({"capabilities":["embedding"]});
        assert!(!apply_show(&mut models[1], &embed));
    }

    #[test]
    fn parses_ps() {
        let ps = json!({"models":[{"name":"a:1","model":"a:1","size":5000u64,"size_vram":4000u64,"context_length":8192}]});
        let r = parse_ps(&ps);
        assert_eq!(r[0].0, "a:1");
        assert_eq!(r[0].1.size_vram_bytes, Some(4000));
    }

    #[test]
    fn accumulates_stream_and_reports_provider_metrics() {
        let mut acc = ChatAccumulator::default();
        let lines = [
            r#"{"message":{"role":"assistant","content":"Hel"},"done":false}"#,
            r#"{"message":{"role":"assistant","content":"lo","thinking":"hm"},"done":false}"#,
            r#"{"message":{"role":"assistant","content":""},"done":true,"done_reason":"stop","load_duration":2000000000,"prompt_eval_count":50,"eval_count":40,"eval_duration":2000000000}"#,
        ];
        for (i, l) in lines.iter().enumerate() {
            acc.ingest_line(l, Duration::from_millis(100 * (i as u64 + 1))).unwrap();
        }
        assert!(acc.done);
        let r = acc.into_result("m", "ollama", Duration::from_secs(3));
        assert_eq!(r.text, "Hello");
        assert_eq!(r.reasoning.as_deref(), Some("hm"));
        assert_eq!(r.completion_tokens, Some(40));
        assert_eq!(r.total_tokens, Some(90));
        assert_eq!(r.tokens_per_second, Some(20.0));
        assert_eq!(r.time_to_first_token_ms, Some(100));
        assert_eq!(r.load_duration_ms, Some(2000));
        assert_eq!(r.finish_reason, Some(FinishReason::Stop));
        assert_eq!(r.metric_source, Some(MetricSource::ProviderReported));
    }

    #[test]
    fn missing_metrics_stay_none() {
        let mut acc = ChatAccumulator::default();
        acc.ingest_line(r#"{"message":{"content":"x"},"done":true}"#, Duration::from_millis(5)).unwrap();
        let r = acc.into_result("m", "ollama", Duration::from_millis(10));
        assert_eq!(r.tokens_per_second, None);
        assert_eq!(r.completion_tokens, None);
        assert_eq!(r.metric_source, None);
    }

    #[test]
    fn stream_error_line_is_reported() {
        let mut acc = ChatAccumulator::default();
        let e = acc.ingest_line(r#"{"error":"model requires more system memory (10 GiB)"}"#, Duration::ZERO).unwrap_err();
        assert!(matches!(e, ProviderError::InsufficientMemory(_)));
    }
}
