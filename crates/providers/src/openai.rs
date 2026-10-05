//! OpenAI-compatible provider (LM Studio, llama.cpp server, vLLM, OpenRouter, ...).
//!
//! Uses `GET /models` and streaming `POST /chat/completions`. When the server is LM Studio, the
//! richer `GET /api/v0/models` endpoint is used for architecture, quantization and context length.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::StreamExt;
use pulsebench_types::{
    FinishReason, GenerationProgress, GenerationRequest, GenerationResult, GenerationSettings, MetricSource, ModelInfo,
    ProviderCapabilities, ProviderConfig, ProviderKind,
};
use reqwest::{Client, RequestBuilder};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::error::ProviderError;
use crate::http::{build_client, error_from_response, join, map_reqwest, LineSplitter};
use crate::{Detection, InferenceProvider, ProgressFn, Result};

pub struct OpenAiCompatProvider {
    config: ProviderConfig,
    client: Client,
}

impl OpenAiCompatProvider {
    pub fn new(config: ProviderConfig) -> Self {
        let client = build_client(&config.base_url);
        Self { config, client }
    }

    fn url(&self, path: &str) -> String {
        join(&self.config.base_url, path)
    }

    /// Server root without a trailing `/v1`, used for vendor-specific endpoints.
    fn root_url(&self) -> String {
        let b = self.config.base_url.trim_end_matches('/');
        b.strip_suffix("/v1").unwrap_or(b).to_string()
    }

    fn auth(&self, rb: RequestBuilder) -> RequestBuilder {
        match &self.config.api_key {
            Some(k) if !k.is_empty() => rb.bearer_auth(k),
            _ => rb,
        }
    }

    async fn get_json(&self, url: &str, timeout: Duration) -> Result<Value> {
        let resp = self.auth(self.client.get(url)).timeout(timeout).send().await.map_err(|e| map_reqwest(&self.config.base_url, e))?;
        if !resp.status().is_success() {
            return Err(error_from_response("", resp).await);
        }
        resp.json::<Value>().await.map_err(|e| ProviderError::InvalidResponse(e.to_string()))
    }

    async fn fetch_models(&self) -> Result<Vec<ModelInfo>> {
        let plain = self.get_json(&self.url("/models"), Duration::from_secs(10)).await?;
        let mut models = parse_models(&self.config, &plain)?;
        // LM Studio exposes richer metadata on a vendor endpoint; ignore failures.
        let v0 = self.get_json(&format!("{}/api/v0/models", self.root_url()), Duration::from_secs(5)).await;
        if let Ok(v0) = v0 {
            merge_lmstudio_v0(&mut models, &v0);
        }
        Ok(models)
    }
}

#[async_trait]
impl InferenceProvider for OpenAiCompatProvider {
    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            honors_seed: true,
            // The OpenAI chat API has no per-request context size; the server's load-time value applies.
            honors_context_length: false,
            reports_token_counts: true,
            reports_timing: false,
        }
    }

    async fn detect(&self) -> Result<Detection> {
        let started = Instant::now();
        self.get_json(&self.url("/models"), Duration::from_secs(3)).await?;
        Ok(Detection { version: None, latency_ms: started.elapsed().as_millis() as u32 })
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        self.fetch_models().await
    }

    async fn model_info(&self, model: &str) -> Result<ModelInfo> {
        let all = self.fetch_models().await?;
        all.into_iter().find(|m| m.id == model).ok_or_else(|| ProviderError::ModelNotFound(model.to_string()))
    }

    async fn generate(
        &self,
        request: &GenerationRequest,
        cancel: &CancellationToken,
        on_progress: ProgressFn<'_>,
    ) -> Result<GenerationResult> {
        let s = &request.settings;
        let mut body = json!({
            "model": request.model,
            "messages": [
                { "role": "system", "content": request.system },
                { "role": "user", "content": request.prompt },
            ],
            "temperature": s.temperature,
            "max_tokens": s.max_output_tokens,
            "stream": true,
            "stream_options": { "include_usage": true },
        });
        if let Some(seed) = s.seed {
            body["seed"] = json!(seed);
        }

        let started = Instant::now();
        let deadline = Duration::from_secs(s.timeout_seconds as u64);
        let work = async {
            let resp = self
                .auth(self.client.post(self.url("/chat/completions")))
                .json(&body)
                .send()
                .await
                .map_err(|e| map_reqwest(&self.config.base_url, e))?;
            if !resp.status().is_success() {
                return Err(error_from_response(&request.model, resp).await);
            }
            let mut acc = SseAccumulator::default();
            let mut splitter = LineSplitter::default();
            let mut stream = resp.bytes_stream();
            'outer: while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| ProviderError::Stream(e.to_string()))?;
                for line in splitter.push(&chunk) {
                    match acc.ingest_line(&line, started.elapsed())? {
                        SseEvent::Output => on_progress(acc.progress(started.elapsed())),
                        SseEvent::Done => break 'outer,
                        SseEvent::Nothing => {}
                    }
                }
            }
            if let Some(line) = splitter.finish() {
                acc.ingest_line(&line, started.elapsed())?;
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
        if acc.chunks == 0 && acc.finish_reason.is_none() {
            return Err(ProviderError::InvalidResponse("the stream contained no completion data".into()));
        }
        Ok(acc.into_result(&request.model, &self.config.id, started.elapsed()))
    }

    async fn warm_up(&self, model: &str, settings: &GenerationSettings) -> Result<()> {
        // Any one-token completion makes JIT-loading servers load the model.
        let body = json!({
            "model": model,
            "messages": [{ "role": "user", "content": "hi" }],
            "max_tokens": 1,
            "temperature": 0,
        });
        let resp = self
            .auth(self.client.post(self.url("/chat/completions")))
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

pub(crate) fn parse_models(cfg: &ProviderConfig, v: &Value) -> Result<Vec<ModelInfo>> {
    let data =
        v.get("data").and_then(Value::as_array).ok_or_else(|| ProviderError::InvalidResponse("/models: missing `data` array".into()))?;
    Ok(data
        .iter()
        .filter_map(|m| m.get("id").and_then(Value::as_str))
        .map(|id| ModelInfo {
            provider_id: cfg.id.clone(),
            provider_kind: ProviderKind::OpenaiCompatible,
            id: id.to_string(),
            display_name: id.to_string(),
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

/// Merge LM Studio's `/api/v0/models` metadata and drop embedding models.
pub(crate) fn merge_lmstudio_v0(models: &mut Vec<ModelInfo>, v0: &Value) {
    let Some(data) = v0.get("data").and_then(Value::as_array) else { return };
    let mut embeddings: Vec<String> = vec![];
    for entry in data {
        let Some(id) = entry.get("id").and_then(Value::as_str) else { continue };
        if entry.get("type").and_then(Value::as_str) == Some("embeddings") {
            embeddings.push(id.to_string());
            continue;
        }
        if let Some(m) = models.iter_mut().find(|m| m.id == id) {
            m.architecture = entry.get("arch").and_then(Value::as_str).map(str::to_string);
            m.family = m.architecture.clone();
            m.quantization = entry.get("quantization").and_then(Value::as_str).map(str::to_string);
            m.context_length = entry.get("max_context_length").and_then(Value::as_u64);
            m.format = entry.get("compatibility_type").and_then(Value::as_str).map(str::to_string);
            m.loaded = entry.get("state").and_then(Value::as_str).map(|s| s == "loaded");
        }
    }
    models.retain(|m| !embeddings.contains(&m.id));
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SseEvent {
    Output,
    Done,
    Nothing,
}

#[derive(Default)]
pub(crate) struct SseAccumulator {
    text: String,
    reasoning: String,
    pub(crate) chunks: u32,
    first_token: Option<Duration>,
    pub(crate) finish_reason: Option<String>,
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
    stats_tps: Option<f64>,
}

impl SseAccumulator {
    pub(crate) fn ingest_line(&mut self, line: &str, elapsed: Duration) -> Result<SseEvent> {
        let line = line.trim();
        let Some(payload) = line.strip_prefix("data:") else {
            // SSE comments / event names / blank keep-alives.
            return Ok(SseEvent::Nothing);
        };
        let payload = payload.trim();
        if payload == "[DONE]" {
            return Ok(SseEvent::Done);
        }
        let v: Value = serde_json::from_str(payload).map_err(|e| ProviderError::InvalidResponse(format!("bad SSE payload: {e}")))?;
        if let Some(err) = v.get("error") {
            let msg = err.get("message").and_then(Value::as_str).unwrap_or("unknown error").to_string();
            let lower = msg.to_lowercase();
            return Err(if lower.contains("memory") { ProviderError::InsufficientMemory(msg) } else { ProviderError::Stream(msg) });
        }
        let mut produced = false;
        if let Some(choice) = v.get("choices").and_then(Value::as_array).and_then(|c| c.first()) {
            if let Some(delta) = choice.get("delta") {
                if let Some(c) = delta.get("content").and_then(Value::as_str) {
                    if !c.is_empty() {
                        self.text.push_str(c);
                        produced = true;
                    }
                }
                let r = delta.get("reasoning_content").or_else(|| delta.get("reasoning")).and_then(Value::as_str);
                if let Some(r) = r {
                    if !r.is_empty() {
                        self.reasoning.push_str(r);
                        produced = true;
                    }
                }
            }
            if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) {
                self.finish_reason = Some(fr.to_string());
            }
        }
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            self.prompt_tokens = u.get("prompt_tokens").and_then(Value::as_u64).map(|n| n as u32).or(self.prompt_tokens);
            self.completion_tokens = u.get("completion_tokens").and_then(Value::as_u64).map(|n| n as u32).or(self.completion_tokens);
            self.total_tokens = u.get("total_tokens").and_then(Value::as_u64).map(|n| n as u32).or(self.total_tokens);
        }
        if let Some(tps) = v.get("stats").and_then(|s| s.get("tokens_per_second")).and_then(Value::as_f64) {
            self.stats_tps = Some(tps);
        }
        if produced {
            self.chunks += 1;
            self.first_token.get_or_insert(elapsed);
            Ok(SseEvent::Output)
        } else {
            Ok(SseEvent::Nothing)
        }
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
        // Prefer the server's own rate; otherwise derive it from usage + wall-clock after first token.
        let (tps, source) = if let Some(t) = self.stats_tps {
            (Some(t), Some(MetricSource::ProviderReported))
        } else if let (Some(n), Some(first)) = (self.completion_tokens, self.first_token) {
            let secs = (elapsed.saturating_sub(first)).as_secs_f64();
            // Tokens after the first one were generated during `secs`.
            if secs > 0.05 && n > 1 {
                (Some((n - 1) as f64 / secs), Some(MetricSource::ClientMeasured))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        GenerationResult {
            text: self.text,
            reasoning: if self.reasoning.is_empty() { None } else { Some(self.reasoning) },
            model: model.to_string(),
            provider: provider.to_string(),
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            total_tokens: self.total_tokens,
            duration_ms: elapsed.as_millis() as u32,
            time_to_first_token_ms: self.first_token.map(|d| d.as_millis() as u32),
            tokens_per_second: tps,
            load_duration_ms: None,
            finish_reason: self.finish_reason.as_deref().map(|r| match r {
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

    fn cfg() -> ProviderConfig {
        ProviderConfig::defaults().remove(1)
    }

    #[test]
    fn parses_models_and_merges_lmstudio_metadata() {
        let plain = json!({"data":[{"id":"qwen/qwen3-coder-30b"},{"id":"text-embedding-nomic"}]});
        let mut models = parse_models(&cfg(), &plain).unwrap();
        let v0 = json!({"data":[
            {"id":"qwen/qwen3-coder-30b","type":"llm","arch":"qwen3_moe","quantization":"Q4_K_M","max_context_length":262144,"state":"loaded","compatibility_type":"gguf"},
            {"id":"text-embedding-nomic","type":"embeddings"}
        ]});
        merge_lmstudio_v0(&mut models, &v0);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].quantization.as_deref(), Some("Q4_K_M"));
        assert_eq!(models[0].context_length, Some(262144));
        assert_eq!(models[0].loaded, Some(true));
    }

    #[test]
    fn plain_models_without_vendor_endpoint() {
        let plain = json!({"data":[{"id":"gpt-x"}]});
        let models = parse_models(&cfg(), &plain).unwrap();
        assert_eq!(models[0].quantization, None);
        assert!(parse_models(&cfg(), &json!({"nope":1})).is_err());
    }

    #[test]
    fn accumulates_sse_with_usage() {
        let mut acc = SseAccumulator::default();
        let lines = [
            ": keep-alive",
            r#"data: {"choices":[{"delta":{"role":"assistant","content":"He"}}]}"#,
            r#"data: {"choices":[{"delta":{"content":"llo"}}]}"#,
            r#"data: {"choices":[{"delta":{},"finish_reason":"length"}]}"#,
            r#"data: {"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":21,"total_tokens":31}}"#,
            "data: [DONE]",
        ];
        let mut last = SseEvent::Nothing;
        for (i, l) in lines.iter().enumerate() {
            last = acc.ingest_line(l, Duration::from_millis(500 + i as u64 * 100)).unwrap();
        }
        assert_eq!(last, SseEvent::Done);
        let r = acc.into_result("m", "lmstudio", Duration::from_millis(2500));
        assert_eq!(r.text, "Hello");
        assert_eq!(r.completion_tokens, Some(21));
        assert_eq!(r.finish_reason, Some(FinishReason::Length));
        assert_eq!(r.time_to_first_token_ms, Some(600));
        // (21-1) tokens over (2.5s - 0.6s)
        let tps = r.tokens_per_second.unwrap();
        assert!((tps - 20.0 / 1.9).abs() < 1e-6);
        assert_eq!(r.metric_source, Some(MetricSource::ClientMeasured));
    }

    #[test]
    fn without_usage_tokens_stay_null() {
        let mut acc = SseAccumulator::default();
        acc.ingest_line(r#"data: {"choices":[{"delta":{"content":"x"},"finish_reason":"stop"}]}"#, Duration::from_millis(10)).unwrap();
        let r = acc.into_result("m", "p", Duration::from_secs(1));
        assert_eq!(r.completion_tokens, None);
        assert_eq!(r.tokens_per_second, None);
    }
}
