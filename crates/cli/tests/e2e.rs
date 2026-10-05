//! Acceptance flow through the real `pulsebench` binary:
//! detect provider -> list models -> run Quick on two models -> leaderboard -> history persists
//! across processes -> export JSON / Markdown / SVG.
//!
//! The model server is an in-process mock of Ollama's HTTP API (the provider code, engine,
//! sandbox, test execution and storage are all real). It answers with the reference solution
//! for one model and with a plausible-but-wrong answer for the other.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;

use axum::body::Body;
use axum::http::header;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn solution_for(title: &str) -> Option<String> {
    let tasks = repo_root().join("benchmarks/quick/tasks");
    for e in std::fs::read_dir(tasks).ok()?.flatten() {
        let tj: Value = serde_json::from_str(&std::fs::read_to_string(e.path().join("task.json")).ok()?).ok()?;
        if tj["title"] == title {
            let editable = tj.get("editableFiles").or_else(|| tj.get("entryFiles"))?.as_array()?.clone();
            let changes: Vec<Value> = editable
                .iter()
                .filter_map(|p| {
                    let p = p.as_str()?;
                    let content = std::fs::read_to_string(e.path().join("solution").join(p)).ok()?;
                    Some(json!({"path": p, "content": content}))
                })
                .collect();
            return Some(json!({"changes": changes, "explanation": "reference"}).to_string());
        }
    }
    None
}

async fn chat(Json(body): Json<Value>) -> Response {
    let msgs = body["messages"].as_array().cloned().unwrap_or_default();
    let ndjson = |lines: Vec<Value>| {
        Response::builder()
            .header(header::CONTENT_TYPE, "application/x-ndjson")
            .body(Body::from(lines.iter().map(|l| l.to_string() + "\n").collect::<String>()))
            .unwrap()
    };
    if msgs.is_empty() {
        // warm-up / unload
        return ndjson(vec![json!({"model": body["model"], "done": true, "done_reason": "load"})]);
    }
    let user = msgs.last().unwrap()["content"].as_str().unwrap_or("");
    let title = user.lines().next().unwrap_or("").trim_start_matches("# Task: ");
    let text = if body["model"] == "oracle:latest" {
        solution_for(title).unwrap_or_default()
    } else {
        // Plausible but wrong: syntactically valid JSON that rewrites the first modifiable file with a stub.
        let path = user
            .lines()
            .skip_while(|l| !l.starts_with("## Files you may modify"))
            .nth(1)
            .unwrap_or("- x")
            .trim_start_matches("- ")
            .to_string();
        json!({"changes":[{"path": path, "content": "# stub\n"}]}).to_string()
    };
    let n = (text.len() / 4).max(1) as u64;
    ndjson(vec![
        json!({"message": {"role": "assistant", "content": text}, "done": false}),
        json!({"message": {"role": "assistant", "content": ""}, "done": true, "done_reason": "stop",
               "prompt_eval_count": 500, "eval_count": n, "eval_duration": n * 25_000_000u64, "load_duration": 1_000_000u64}),
    ])
}

async fn serve() -> SocketAddr {
    let app = Router::new()
        .route("/api/version", get(|| async { Json(json!({"version": "0.99.0-mock"})) }))
        .route(
            "/api/tags",
            get(|| async {
                let m = |n: &str| json!({"name": n, "model": n, "size": 4_000_000_000u64, "details": {"format": "gguf", "family": "qwen2", "parameter_size": "7B", "quantization_level": "Q4_K_M"}});
                Json(json!({"models": [m("oracle:latest"), m("sloppy:latest")]}))
            }),
        )
        .route("/api/show", post(|| async { Json(json!({"capabilities": ["completion"], "model_info": {"general.architecture": "qwen2", "qwen2.context_length": 32768}})) }))
        .route("/api/ps", get(|| async { Json(json!({"models": []})) }))
        .route("/api/chat", post(chat));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

fn pulsebench(data: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_pulsebench"))
        .env("PULSEBENCH_DATA", data)
        .env("PULSEBENCH_SUITES", repo_root().join("benchmarks"))
        .env("PULSEBENCH_NODE_CACHE_HINT", "unused")
        .args(args)
        .output()
        .unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn full_acceptance_flow_through_the_cli() {
    let addr = serve().await;
    let data = tempfile::tempdir().unwrap();

    // Point the Ollama provider at the mock by writing settings the same way the app does.
    {
        let store = pulsebench_storage::Store::open(&data.path().join("pulsebench.db")).unwrap();
        let mut s = store.load_settings().unwrap();
        s.providers[0].base_url = format!("http://{addr}");
        s.providers[1].enabled = false;
        store.save_settings(&s).unwrap();
    }
    let data_path = data.path().to_path_buf();

    let dp = data_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        // 1. discovery
        let (code, out, _) = pulsebench(&dp, &["models"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("● Ollama") && out.contains("v0.99.0-mock"), "{out}");
        assert!(out.contains("oracle:latest") && out.contains("Q4_K_M") && out.contains("32768 ctx"), "{out}");

        // 2. run two models on the Quick suite
        let (code, out, err) = pulsebench(&dp, &["run", "quick", "--model", "oracle:latest", "--model", "sloppy:latest"]);
        assert_eq!(code, 0, "stdout:\n{out}\nstderr:\n{err}");
        assert!(out.contains("QUICK CODING"), "{out}");
        let rank_of = |name: &str| {
            out.lines()
                .find(|l| {
                    let mut it = l.split_whitespace();
                    matches!((it.next(), it.next()), (Some(r), Some(n)) if r.len() == 1 && r.chars().all(|c| c.is_ascii_digit()) && n == name)
                })
                .unwrap_or_else(|| panic!("no leaderboard row for {name}:\n{out}"))
                .to_string()
        };
        let first = rank_of("oracle:latest");
        let second = rank_of("sloppy:latest");
        assert!(first.starts_with('1'), "{first}");
        assert!(second.starts_with('2'), "{second}");
        assert!(first.contains("100%"), "{first}");
        assert!(second.contains("0%"), "{second}");
        let run_id = out.lines().find_map(|l| l.strip_prefix("Run ID: ")).unwrap().split_whitespace().next().unwrap().to_string();

        // 3. history survives the process (new process, same data dir)
        let (_, hist, _) = pulsebench(&dp, &["history"]);
        assert!(hist.contains(&run_id), "{hist}");

        // 4. exports
        let (code, json_text, _) = pulsebench(&dp, &["export", &run_id, "--format", "json"]);
        assert_eq!(code, 0);
        let v: Value = serde_json::from_str(&json_text).unwrap();
        assert_eq!(v["schema"], "pulsebench-result-v1");
        assert_eq!(v["models"].as_array().unwrap().len(), 2);
        let tasks = v["models"][0]["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 5);
        assert!(tasks[0]["attempts"][0]["tests"]["command"]["stderr"].as_str().unwrap().contains("Ran"), "real test output must be captured");
        assert!(tasks[0]["attempts"][0]["diff"].as_str().unwrap().contains("@@"));
        let s0 = v["models"][0]["score"]["pulsebenchScore"].as_u64().unwrap();
        let s1 = v["models"][1]["score"]["pulsebenchScore"].as_u64().unwrap();
        assert_ne!(s0, s1, "models are scored independently");

        let (_, md, _) = pulsebench(&dp, &["export", &run_id, "--format", "markdown"]);
        assert!(md.contains("# PulseBench Report") && md.contains("**Winner:** oracle:latest"), "{md}");
        let (_, svg, _) = pulsebench(&dp, &["export", &run_id, "--format", "svg"]);
        assert!(svg.starts_with("<svg") && svg.contains("oracle:latest"));

        // 5. unknown model gives a clear error, not a crash
        let (code, _, err) = pulsebench(&dp, &["run", "quick", "--model", "does-not-exist"]);
        assert_eq!(code, 1);
        assert!(err.contains("not found"), "{err}");
        run_id
    })
    .await
    .unwrap();
    assert!(!result.is_empty());
    // No workspace directories are left behind.
    let work = data_path.join("work");
    assert!(
        !work.exists()
            || std::fs::read_dir(work)
                .unwrap()
                .all(|e| std::fs::read_dir(e.unwrap().path()).map(|mut d| d.next().is_none()).unwrap_or(true))
    );
}
