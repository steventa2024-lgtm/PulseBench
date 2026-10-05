//! Exercises the real Ollama and OpenAI-compatible providers against in-process mock HTTP servers.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use pulsebench_providers::{build_provider, probe, ProviderError};
use pulsebench_types::{ConnectionState, FinishReason, GenerationRequest, GenerationSettings, ProviderConfig, ProviderKind};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

async fn serve(app: Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

fn cfg(id: &str, kind: ProviderKind, addr: SocketAddr, suffix: &str) -> ProviderConfig {
    ProviderConfig { id: id.into(), kind, name: id.into(), base_url: format!("http://{addr}{suffix}"), enabled: true, api_key: None }
}

fn request(model: &str, timeout: u32) -> GenerationRequest {
    GenerationRequest {
        model: model.into(),
        system: "sys".into(),
        prompt: "hello".into(),
        settings: GenerationSettings { timeout_seconds: timeout, ..Default::default() },
    }
}

fn ndjson(lines: &[Value]) -> Response {
    let body = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n") + "\n";
    Response::builder().header(header::CONTENT_TYPE, "application/x-ndjson").body(Body::from(body)).unwrap()
}

fn ollama_app(chat_calls: Arc<AtomicU32>) -> Router {
    Router::new()
        .route("/api/version", get(|| async { Json(json!({"version":"0.12.3"})) }))
        .route(
            "/api/tags",
            get(|| async {
                Json(json!({"models":[
                    {"name":"qwen2.5-coder:7b","model":"qwen2.5-coder:7b","size":4683087332u64,
                     "details":{"format":"gguf","family":"qwen2","parameter_size":"7.6B","quantization_level":"Q4_K_M"}},
                    {"name":"nomic-embed-text:latest","model":"nomic-embed-text:latest","size":274302450u64,"details":{}}
                ]}))
            }),
        )
        .route(
            "/api/show",
            post(|Json(body): Json<Value>| async move {
                if body["model"] == "nomic-embed-text:latest" {
                    Json(json!({"capabilities":["embedding"]}))
                } else {
                    Json(json!({"capabilities":["completion"],"model_info":{"general.architecture":"qwen2","qwen2.context_length":32768}}))
                }
            }),
        )
        .route("/api/ps", get(|| async { Json(json!({"models":[{"name":"qwen2.5-coder:7b","model":"qwen2.5-coder:7b","size":6000000000u64,"size_vram":5500000000u64}]})) }))
        .route(
            "/api/chat",
            post(|State(calls): State<Arc<AtomicU32>>, Json(body): Json<Value>| async move {
                calls.fetch_add(1, Ordering::SeqCst);
                if body["model"] == "missing" {
                    return Response::builder().status(404).body(Body::from(r#"{"error":"model 'missing' not found"}"#)).unwrap();
                }
                if body["model"] == "slow" {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
                // Verify the standardized options reach the server.
                assert_eq!(body["options"]["temperature"], 0.0);
                assert_eq!(body["options"]["seed"], 42);
                ndjson(&[
                    json!({"message":{"role":"assistant","content":"{\"changes\":"},"done":false}),
                    json!({"message":{"role":"assistant","content":"[]}"},"done":false}),
                    json!({"message":{"role":"assistant","content":""},"done":true,"done_reason":"stop",
                           "load_duration":1000000000u64,"prompt_eval_count":12,"eval_count":30,"eval_duration":1500000000u64}),
                ])
            }),
        )
        .with_state(chat_calls)
}

#[tokio::test]
async fn ollama_discovery_enriches_models_and_hides_embeddings() {
    let addr = serve(ollama_app(Arc::new(AtomicU32::new(0)))).await;
    let p = build_provider(&cfg("ollama", ProviderKind::Ollama, addr, ""));
    let status = probe(p.as_ref()).await;
    assert_eq!(status.state, ConnectionState::Connected);
    assert_eq!(status.version.as_deref(), Some("0.12.3"));
    assert_eq!(status.models.len(), 1, "embedding-only model must be hidden");
    let m = &status.models[0];
    assert_eq!(m.id, "qwen2.5-coder:7b");
    assert_eq!(m.context_length, Some(32768));
    assert_eq!(m.quantization.as_deref(), Some("Q4_K_M"));
    assert_eq!(m.loaded, Some(true));
    let res = p.residency("qwen2.5-coder:7b").await.unwrap().unwrap();
    assert_eq!(res.size_vram_bytes, Some(5_500_000_000));
}

#[tokio::test]
async fn ollama_generate_streams_and_reports_provider_metrics() {
    let calls = Arc::new(AtomicU32::new(0));
    let addr = serve(ollama_app(calls.clone())).await;
    let p = build_provider(&cfg("ollama", ProviderKind::Ollama, addr, ""));
    let progress = Arc::new(AtomicU32::new(0));
    let pc = progress.clone();
    let r = p
        .generate(&request("qwen2.5-coder:7b", 30), &CancellationToken::new(), &move |_| {
            pc.fetch_add(1, Ordering::SeqCst);
        })
        .await
        .unwrap();
    assert_eq!(r.text, "{\"changes\":[]}");
    assert_eq!(r.completion_tokens, Some(30));
    assert_eq!(r.tokens_per_second, Some(20.0));
    assert_eq!(r.finish_reason, Some(FinishReason::Stop));
    assert_eq!(r.load_duration_ms, Some(1000));
    assert!(progress.load(Ordering::SeqCst) >= 2);
}

#[tokio::test]
async fn ollama_errors_are_typed() {
    let addr = serve(ollama_app(Arc::new(AtomicU32::new(0)))).await;
    let p = build_provider(&cfg("ollama", ProviderKind::Ollama, addr, ""));
    let err = p.generate(&request("missing", 5), &CancellationToken::new(), &|_| {}).await.unwrap_err();
    assert!(matches!(err, ProviderError::ModelNotFound(_)), "{err:?}");

    let err = p.generate(&request("slow", 1), &CancellationToken::new(), &|_| {}).await.unwrap_err();
    assert!(err.is_timeout(), "{err:?}");

    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        c2.cancel();
    });
    let err = p.generate(&request("slow", 30), &cancel, &|_| {}).await.unwrap_err();
    assert!(err.is_cancelled(), "{err:?}");
}

#[tokio::test]
async fn unreachable_provider_is_reported_not_panicked() {
    // Port 9 (discard) on loopback is almost certainly closed.
    let c = ProviderConfig {
        id: "ollama".into(),
        kind: ProviderKind::Ollama,
        name: "Ollama".into(),
        base_url: "http://127.0.0.1:9".into(),
        enabled: true,
        api_key: None,
    };
    let status = probe(build_provider(&c).as_ref()).await;
    assert_eq!(status.state, ConnectionState::Unreachable);
    assert!(status.error.unwrap().contains("cannot connect"));
}

fn openai_app() -> Router {
    Router::new()
        .route("/v1/models", get(|| async { Json(json!({"data":[{"id":"qwen/qwen3-coder-30b"},{"id":"nomic-embed"}]})) }))
        .route(
            "/api/v0/models",
            get(|| async {
                Json(json!({"data":[
                    {"id":"qwen/qwen3-coder-30b","type":"llm","arch":"qwen3_moe","quantization":"Q4_K_M","max_context_length":262144,"state":"not-loaded"},
                    {"id":"nomic-embed","type":"embeddings"}]}))
            }),
        )
        .route(
            "/v1/chat/completions",
            post(|Json(body): Json<Value>| async move {
                assert_eq!(body["stream"], true);
                assert_eq!(body["temperature"], 0.0);
                let sse = [
                    r#"data: {"choices":[{"delta":{"content":"ok"}}]}"#,
                    r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
                    r#"data: {"choices":[],"usage":{"prompt_tokens":9,"completion_tokens":40,"total_tokens":49}}"#,
                    "data: [DONE]",
                ]
                .join("\n\n")
                    + "\n\n";
                tokio::time::sleep(Duration::from_millis(150)).await;
                Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, "text/event-stream").body(Body::from(sse)).unwrap()
            }),
        )
}

#[tokio::test]
async fn openai_compatible_discovery_and_generation() {
    let addr = serve(openai_app()).await;
    let p = build_provider(&cfg("lmstudio", ProviderKind::OpenaiCompatible, addr, "/v1"));
    let status = probe(p.as_ref()).await;
    assert_eq!(status.state, ConnectionState::Connected);
    assert_eq!(status.models.len(), 1);
    assert_eq!(status.models[0].context_length, Some(262144));

    let r = p.generate(&request("qwen/qwen3-coder-30b", 30), &CancellationToken::new(), &|_| {}).await.unwrap();
    assert_eq!(r.text, "ok");
    assert_eq!(r.completion_tokens, Some(40));
    assert!(r.time_to_first_token_ms.is_some());
}
