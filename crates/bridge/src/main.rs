//! Headless HTTP bridge to the PulseBench command surface.
//!
//! Used to develop and test the web UI without the desktop shell (`npm run dev:bridge`, Playwright
//! end-to-end tests). It exposes exactly the commands the desktop app exposes, backed by the very
//! same `App` service. It only binds to loopback and is not part of the shipped application.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use clap::Parser;
use futures_util::StreamExt;
use pulsebench_app::{rpc, App, AppPaths};
use serde_json::{json, Value};
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::CorsLayer;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8787")]
    listen: SocketAddr,
    #[arg(long, env = "PULSEBENCH_DATA")]
    data_dir: Option<PathBuf>,
    #[arg(long, env = "PULSEBENCH_SUITES")]
    suites_dir: Option<PathBuf>,
}

async fn call(State(app): State<Arc<App>>, Path(cmd): Path<String>, Json(args): Json<Value>) -> (axum::http::StatusCode, Json<Value>) {
    match rpc::dispatch(&app, &cmd, args).await {
        Ok(v) => (axum::http::StatusCode::OK, Json(v)),
        Err(e) => (axum::http::StatusCode::BAD_REQUEST, Json(json!({ "error": e }))),
    }
}

async fn events(State(app): State<Arc<App>>) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(app.events()).filter_map(|r| async move {
        let ev = r.ok()?;
        Some(Ok(Event::default().data(serde_json::to_string(&ev).ok()?)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_target(false).init();
    let args = Args::parse();
    if !args.listen.ip().is_loopback() {
        eprintln!("refusing to bind to a non-loopback address: the bridge has no authentication");
        std::process::exit(2);
    }
    let app = App::new(AppPaths::detect(args.data_dir, args.suites_dir)).await.expect("start app");
    let router =
        Router::new().route("/rpc/{cmd}", post(call)).route("/events", get(events)).layer(CorsLayer::very_permissive()).with_state(app);
    let listener = tokio::net::TcpListener::bind(args.listen).await.expect("bind");
    println!("PulseBench bridge listening on http://{}", args.listen);
    axum::serve(listener, router).await.expect("serve");
}
