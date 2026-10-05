//! One JSON command surface shared by the desktop shell and the headless bridge.
//!
//! Each command takes a JSON object of named arguments and returns a JSON value. The TypeScript
//! side wraps every command in a typed function (see `apps/desktop/src/ipc/commands.ts`), so
//! the untyped transport never leaks into UI code.

use std::path::PathBuf;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::types::{ExportFormat, StartRunRequest};
use crate::App;

fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T, String> {
    let v = args.get(key).cloned().unwrap_or(Value::Null);
    serde_json::from_value(v).map_err(|e| format!("invalid argument `{key}`: {e}"))
}

fn ok<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Names of all commands (used by tests to keep the TypeScript wrapper in sync).
pub const COMMANDS: &[&str] = &[
    "get_data_info",
    "default_run_settings",
    "get_system_info",
    "live_sample",
    "get_settings",
    "save_settings",
    "mark_onboarding_done",
    "provider_statuses",
    "list_suites",
    "import_suite",
    "remove_custom_suite",
    "prepare_suite",
    "start_run",
    "cancel_run",
    "active_run",
    "list_runs",
    "get_run",
    "delete_run",
    "clear_history",
    "model_history",
    "latest_scores",
    "export_run",
    "export_run_to_file",
    "export_database",
    "save_png",
];

pub async fn dispatch(app: &Arc<App>, cmd: &str, args: Value) -> Result<Value, String> {
    match cmd {
        "get_data_info" => ok(app.data_info().map_err(err)?),
        "default_run_settings" => ok(pulsebench_types::RunSettings::default()),
        "get_system_info" => ok(app.system_info(arg::<Option<bool>>(&args, "refresh")?.unwrap_or(false)).await),
        "live_sample" => ok(app.live_sample().await),
        "get_settings" => ok(app.settings().map_err(err)?),
        "save_settings" => ok(app.save_settings(arg(&args, "settings")?).map_err(err)?),
        "mark_onboarding_done" => {
            app.mark_onboarding_done().map_err(err)?;
            ok(json!(null))
        }
        "provider_statuses" => ok(app.provider_statuses().await.map_err(err)?),
        "list_suites" => ok(app.suites().map_err(err)?),
        "import_suite" => ok(app.import_suite(&PathBuf::from(arg::<String>(&args, "path")?)).map_err(err)?),
        "remove_custom_suite" => {
            app.remove_custom_suite(&arg::<String>(&args, "id")?).map_err(err)?;
            ok(json!(null))
        }
        "prepare_suite" => ok(app.prepare_suite(&arg::<String>(&args, "suiteId")?, &CancellationToken::new()).await.map_err(err)?),
        "start_run" => {
            let req: StartRunRequest = arg(&args, "request")?;
            ok(app.start_run(req).await.map_err(err)?)
        }
        "cancel_run" => ok(app.cancel_run()),
        "active_run" => ok(app.active_run()),
        "list_runs" => ok(app.list_runs(arg::<Option<u32>>(&args, "limit")?.unwrap_or(100)).map_err(err)?),
        "get_run" => ok(app.get_run(&arg::<String>(&args, "id")?).map_err(err)?),
        "delete_run" => {
            app.delete_run(&arg::<String>(&args, "id")?).map_err(err)?;
            ok(json!(null))
        }
        "clear_history" => ok(app.clear_history().map_err(err)?),
        "model_history" => {
            ok(app.model_history(&arg::<String>(&args, "modelKey")?, arg::<Option<String>>(&args, "suiteId")?.as_deref()).map_err(err)?)
        }
        "latest_scores" => ok(app.latest_scores().map_err(err)?),
        "export_run" => {
            let fmt: ExportFormat = arg(&args, "format")?;
            ok(app.export_run(&arg::<String>(&args, "id")?, fmt, arg::<Option<String>>(&args, "modelKey")?.as_deref()).map_err(err)?)
        }
        "export_run_to_file" => {
            let fmt: ExportFormat = arg(&args, "format")?;
            app.export_run_to_file(
                &arg::<String>(&args, "id")?,
                fmt,
                arg::<Option<String>>(&args, "modelKey")?.as_deref(),
                &PathBuf::from(arg::<String>(&args, "path")?),
            )
            .map_err(err)?;
            ok(json!(null))
        }
        "export_database" => {
            app.export_database(&PathBuf::from(arg::<String>(&args, "path")?)).map_err(err)?;
            ok(json!(null))
        }
        "save_png" => {
            use base64::Engine;
            let path = PathBuf::from(arg::<String>(&args, "path")?);
            if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("png")) != Some(true) {
                return Err("only .png files can be written with save_png".into());
            }
            let bytes = base64::engine::general_purpose::STANDARD.decode(arg::<String>(&args, "base64")?).map_err(err)?;
            if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
                return Err("data is not a PNG image".into());
            }
            std::fs::write(&path, bytes).map_err(err)?;
            ok(json!(null))
        }
        other => Err(format!("unknown command `{other}`")),
    }
}
