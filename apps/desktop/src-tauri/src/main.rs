// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use pulsebench_app::{rpc, App, AppPaths};
use tauri::{Emitter, Manager, WindowEvent};
use tokio::sync::broadcast::error::RecvError;

/// The single IPC entry point. The TypeScript side wraps each command in a typed function
/// (`apps/desktop/src/ipc/commands.ts`); the logic lives in `pulsebench-app`.
#[tauri::command]
async fn rpc(app: tauri::State<'_, Arc<App>>, cmd: String, args: serde_json::Value) -> Result<serde_json::Value, String> {
    rpc::dispatch(&app, &cmd, args).await
}

fn main() {
    tracing_subscriber::fmt().with_target(false).init();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let bundled = app.path().resource_dir().ok().map(|d| d.join("benchmarks"));
            let core = tauri::async_runtime::block_on(App::new(AppPaths::detect(None, bundled)))?;

            // Forward engine events (progress, logs, telemetry) to the web view.
            let mut rx = core.events();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(ev) => {
                            let _ = handle.emit("pulsebench://event", ev);
                        }
                        Err(RecvError::Lagged(_)) => continue,
                        Err(RecvError::Closed) => break,
                    }
                }
            });
            app.manage(core);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window cancels a running benchmark and waits for child processes to be reaped.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let core = window.app_handle().state::<Arc<App>>().inner().clone();
                if core.active_run().is_some() {
                    api.prevent_close();
                    let w = window.clone();
                    tauri::async_runtime::spawn(async move {
                        core.shutdown().await;
                        let _ = w.destroy();
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![rpc])
        .run(tauri::generate_context!())
        .expect("error while running PulseBench");
}
