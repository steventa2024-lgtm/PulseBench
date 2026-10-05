//! Service-level tests that need no model server: settings, suites, RPC surface.

use std::path::PathBuf;

use pulsebench_app::{rpc, App, AppPaths};
use serde_json::json;

fn benchmarks() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks")
}

async fn app(dir: &std::path::Path) -> std::sync::Arc<App> {
    App::new(AppPaths { data_dir: dir.to_path_buf(), bundled_suites: vec![benchmarks()] }).await.unwrap()
}

#[tokio::test]
async fn settings_mask_api_keys_and_keep_them_across_saves() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path()).await;
    let mut s = app.settings().unwrap();
    s.providers.push(pulsebench_types::ProviderConfig {
        id: "custom".into(),
        kind: pulsebench_types::ProviderKind::OpenaiCompatible,
        name: "Custom".into(),
        base_url: "http://10.0.0.2:8000/v1/".into(),
        enabled: true,
        api_key: Some("sk-secret".into()),
    });
    let saved = app.save_settings(s).unwrap();
    let custom = saved.providers.iter().find(|p| p.id == "custom").unwrap();
    assert_eq!(custom.api_key.as_deref(), Some("********"), "the UI never receives the real key");
    assert_eq!(custom.base_url, "http://10.0.0.2:8000/v1", "trailing slash is normalized");

    // Saving the masked value back keeps the stored key instead of overwriting it with the mask.
    app.save_settings(saved).unwrap();
    let registry_cfg = pulsebench_storage::Store::open(&dir.path().join("pulsebench.db")).unwrap().load_settings().unwrap();
    assert_eq!(registry_cfg.providers.iter().find(|p| p.id == "custom").unwrap().api_key.as_deref(), Some("sk-secret"));
}

#[tokio::test]
async fn invalid_settings_are_rejected_with_clear_messages() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path()).await;
    let mut s = app.settings().unwrap();
    s.run.generation.temperature = 5.0;
    assert!(app.save_settings(s.clone()).unwrap_err().to_string().contains("temperature"));
    s.run.generation.temperature = 0.0;
    s.providers[0].base_url = "ftp://nope".into();
    assert!(app.save_settings(s.clone()).unwrap_err().to_string().contains("http"));
    s.providers[0].base_url = "http://localhost:11434".into();
    s.providers[1].id = s.providers[0].id.clone();
    assert!(app.save_settings(s).unwrap_err().to_string().contains("duplicate"));
}

#[tokio::test]
async fn bundled_suites_are_listed_and_custom_import_is_validated() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path()).await;
    let list = app.suites().unwrap();
    assert!(list.problems.is_empty(), "{:?}", list.problems);
    let ids: Vec<&str> = list.suites.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"quick") && ids.contains(&"full"));
    assert!(list.suites.iter().all(|s| s.official), "bundled suites are lock-verified");

    // Importing a bundled suite again must fail (id collision), a broken folder must fail clearly.
    assert!(app.import_suite(&benchmarks().join("quick")).unwrap_err().to_string().contains("already exists"));
    let broken = tempfile::tempdir().unwrap();
    std::fs::write(broken.path().join("suite.json"), "{}").unwrap();
    assert!(app.import_suite(broken.path()).is_err());

    // A valid copy under a new id imports and is not "official".
    let copy = tempfile::tempdir().unwrap();
    copy_dir(&benchmarks().join("quick"), copy.path());
    let sj = copy.path().join("suite.json");
    std::fs::write(&sj, std::fs::read_to_string(&sj).unwrap().replace("\"id\": \"quick\"", "\"id\": \"my-quick\"")).unwrap();
    let imported = app.import_suite(copy.path()).unwrap();
    assert_eq!(imported.id, "my-quick");
    assert!(!imported.official, "a modified copy must not claim official status");
    assert!(app.suites().unwrap().suites.iter().any(|s| s.id == "my-quick"));
    app.remove_custom_suite("my-quick").unwrap();
    assert!(app.remove_custom_suite("quick").is_err(), "bundled suites cannot be removed");
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&to).unwrap();
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), &to).unwrap();
        }
    }
}

#[tokio::test]
async fn rpc_surface_is_safe_and_complete() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path()).await;
    assert!(rpc::dispatch(&app, "format_disk", json!({})).await.unwrap_err().contains("unknown command"));
    assert!(rpc::dispatch(&app, "get_run", json!({})).await.is_err());
    assert!(rpc::dispatch(&app, "get_run", json!({"id": "nope"})).await.unwrap_err().contains("not found"));
    for cmd in ["get_settings", "list_suites", "list_runs", "latest_scores", "active_run", "get_data_info", "default_run_settings"] {
        assert!(rpc::dispatch(&app, cmd, json!({})).await.is_ok(), "{cmd}");
    }
    // `save_png` only writes real PNG data to .png files.
    let p = dir.path().join("x.png");
    assert!(rpc::dispatch(&app, "save_png", json!({"path": p, "base64": "aGVsbG8="})).await.unwrap_err().contains("PNG"));
    assert!(rpc::dispatch(&app, "save_png", json!({"path": dir.path().join("x.exe"), "base64": "iVBORw=="})).await.unwrap_err().contains(".png"));
    assert!(!p.exists());
    // Starting a run with no models or an unknown suite is a clear error, never a crash.
    let err = rpc::dispatch(&app, "start_run", json!({"request": {"suiteId": "quick", "models": []}})).await.unwrap_err();
    assert!(err.contains("at least one model"), "{err}");
    let err = rpc::dispatch(&app, "start_run", json!({"request": {"suiteId": "nope", "models": [{"providerId":"ollama","modelId":"x"}]}})).await.unwrap_err();
    assert!(err.contains("not found"), "{err}");
    assert_eq!(app.list_runs(10).unwrap().len(), 0);
    // Every advertised command is routed.
    for cmd in rpc::COMMANDS {
        let r = rpc::dispatch(&app, cmd, json!({})).await;
        if let Err(e) = r {
            assert!(!e.contains("unknown command"), "{cmd} is advertised but not routed");
        }
    }
}
