use pulsebench_types::testing::sample_run;
use pulsebench_types::*;

use super::*;

#[test]
fn migrations_are_applied_and_idempotent() {
    let s = Store::open_in_memory().unwrap();
    assert_eq!(s.schema_version().unwrap(), MIGRATIONS.last().unwrap().0);
    s.migrate_to(i64::MAX).unwrap();
    assert_eq!(s.schema_version().unwrap(), MIGRATIONS.last().unwrap().0);
}

#[test]
fn history_survives_upgrades_between_versions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pulsebench.db");
    // Simulate an install that only ever ran migration 1.
    {
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        let s = Store { conn: Mutex::new(conn) };
        s.migrate_to(1).unwrap();
        assert_eq!(s.schema_version().unwrap(), 1);
        s.save_run(&sample_run("old-run", &[("qwen2.5-coder:7b", 7000)]), true).unwrap();
    }
    // Opening with the current build upgrades in place and keeps the data.
    let s = Store::open(&path).unwrap();
    assert_eq!(s.schema_version().unwrap(), MIGRATIONS.last().unwrap().0);
    let run = s.get_run("old-run").unwrap().expect("old run must survive the upgrade");
    assert_eq!(run.models[0].score.as_ref().unwrap().pulsebench_score, 7000);
}

#[test]
fn run_round_trips_and_is_listed_with_models() {
    let s = Store::open_in_memory().unwrap();
    let run = sample_run("r1", &[("a:1", 9000), ("b:2", 8000)]);
    s.save_run(&run, true).unwrap();
    let back = s.get_run("r1").unwrap().unwrap();
    assert_eq!(back, run);
    let list = s.list_runs(10).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].models.len(), 2);
    assert_eq!(list[0].models[0].score, Some(9000), "models are ordered by score");
}

#[test]
fn checkpoint_then_finalize_does_not_duplicate_rows() {
    let s = Store::open_in_memory().unwrap();
    let mut run = sample_run("r1", &[("a:1", 9000)]);
    run.status = RunStatus::Running;
    s.save_run(&run, false).unwrap();
    assert!(s.list_runs(10).unwrap()[0].models.is_empty(), "checkpoints do not populate query tables");
    run.status = RunStatus::Completed;
    s.save_run(&run, true).unwrap();
    s.save_run(&run, true).unwrap();
    assert_eq!(s.list_runs(10).unwrap()[0].models.len(), 1);
    assert_eq!(s.list_runs(10).unwrap().len(), 1);
}

#[test]
fn interrupted_runs_are_marked_on_startup() {
    let s = Store::open_in_memory().unwrap();
    let mut run = sample_run("r1", &[("a:1", 100)]);
    run.status = RunStatus::Running;
    run.models[0].status = ModelRunStatus::Running;
    s.save_run(&run, false).unwrap();
    assert_eq!(s.mark_interrupted().unwrap(), 1);
    let back = s.get_run("r1").unwrap().unwrap();
    assert_eq!(back.status, RunStatus::Interrupted);
    assert_eq!(back.models[0].status, ModelRunStatus::Cancelled);
    assert_eq!(s.mark_interrupted().unwrap(), 0);
}

#[test]
fn model_history_is_ordered_and_filtered() {
    let s = Store::open_in_memory().unwrap();
    let mut r1 = sample_run("r1", &[("a:1", 8390)]);
    r1.created_at -= chrono::Duration::days(14);
    let r2 = sample_run("r2", &[("a:1", 8742), ("b:2", 5000)]);
    s.save_run(&r1, true).unwrap();
    s.save_run(&r2, true).unwrap();
    let h = s.model_history("ollama/a:1", Some("quick")).unwrap();
    assert_eq!(h.iter().map(|p| p.score.unwrap()).collect::<Vec<_>>(), vec![8390, 8742]);
    assert!(s.model_history("ollama/a:1", Some("full")).unwrap().is_empty());
    let latest = s.latest_scores().unwrap();
    assert_eq!(latest.iter().find(|(k, _, _)| k == "ollama/a:1").unwrap().1, 8742);
}

#[test]
fn settings_round_trip_including_providers() {
    let s = Store::open_in_memory().unwrap();
    let defaults = s.load_settings().unwrap();
    assert_eq!(defaults.providers.len(), 2);
    let mut custom = defaults.clone();
    custom.run.generation.max_output_tokens = 1234;
    custom.keep_workspaces = true;
    custom.providers.push(ProviderConfig {
        id: "custom-1".into(),
        kind: ProviderKind::OpenaiCompatible,
        name: "Mine".into(),
        base_url: "http://10.0.0.5:8000/v1".into(),
        enabled: true,
        api_key: Some("sk-test".into()),
    });
    s.save_settings(&custom).unwrap();
    let back = s.load_settings().unwrap();
    assert_eq!(back, custom);
}

#[test]
fn old_settings_documents_gain_new_fields_via_defaults() {
    let s = Store::open_in_memory().unwrap();
    {
        let conn = s.conn.lock().unwrap();
        conn.execute("INSERT INTO settings (key, value_json, updated_at) VALUES ('app', ?1, 'x')", [r#"{"keepWorkspaces":true}"#]).unwrap();
    }
    let loaded = s.load_settings().unwrap();
    assert!(loaded.keep_workspaces);
    assert_eq!(loaded.run, RunSettings::default());
}

#[test]
fn delete_cascades_and_export_works() {
    let s = Store::open_in_memory().unwrap();
    let mut run = sample_run("r1", &[("a:1", 1)]);
    run.telemetry = vec![TelemetrySeries {
        model_key: "ollama/a:1".into(),
        samples: vec![TelemetrySample {
            ts: Utc::now(),
            cpu_percent: Some(5.0),
            ram_used_mb: 100,
            ram_total_mb: 200,
            gpu_percent: None,
            vram_used_mb: None,
            vram_total_mb: None,
            gpu_temp_c: None,
        }],
    }];
    s.save_run(&run, true).unwrap();
    {
        let conn = s.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM telemetry_samples", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("copy.db");
    s.export_to(&dest).unwrap();
    assert!(Store::open(&dest).unwrap().get_run("r1").unwrap().is_some());
    assert!(s.export_to(&dest).is_err(), "never overwrites an existing file");
    assert!(s.delete_run("r1").unwrap());
    let conn = s.conn.lock().unwrap();
    for table in ["benchmark_models", "task_results", "telemetry_samples"] {
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "{table} should be empty after cascade delete");
    }
}
