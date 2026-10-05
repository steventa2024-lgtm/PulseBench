//! Builders for unit tests in downstream crates. Never used by the application itself.

use chrono::Utc;

use crate::*;

pub fn sample_system() -> SystemInfo {
    SystemInfo {
        os_name: "TestOS".into(),
        os_version: "1.0".into(),
        arch: "x86_64".into(),
        hostname_hash: "abc123".into(),
        cpu_model: "Test CPU".into(),
        cpu_physical_cores: Some(8),
        cpu_logical_cores: 16,
        ram_total_mb: 32768,
        ram_available_mb: 20000,
        gpus: vec![GpuInfo {
            name: "Test GPU".into(),
            vendor: GpuVendor::Nvidia,
            vram_total_mb: Some(8192),
            vram_used_mb: Some(500),
            driver_version: Some("1.0".into()),
            telemetry_available: true,
        }],
        docker: DockerStatus { installed: false, running: false, version: None },
        runtimes: vec![],
    }
}

pub fn sample_model(id: &str) -> ModelInfo {
    ModelInfo {
        provider_id: "ollama".into(),
        provider_kind: ProviderKind::Ollama,
        id: id.into(),
        display_name: id.into(),
        family: None,
        architecture: Some("qwen2".into()),
        parameter_size: Some("7B".into()),
        quantization: Some("Q4_K_M".into()),
        size_bytes: Some(4_000_000_000),
        context_length: Some(32768),
        format: Some("gguf".into()),
        loaded: None,
        modified_at: None,
    }
}

pub fn sample_task(id: &str, status: TaskStatus, score: f64) -> TaskResult {
    TaskResult {
        id: id.into(),
        title: format!("Task {id}"),
        category: Category::Bugfix,
        language: Language::Python,
        difficulty: Difficulty::Easy,
        status,
        failure: None,
        message: None,
        max_attempts: 1,
        attempts: vec![],
        baseline: None,
        score: Some(TaskScore { value: score, components: ComponentScores::default(), weight: 1.0 }),
        started_at: Utc::now(),
        duration_ms: 1500,
    }
}

/// A finished run with the given models, each `(model id, pulsebench score)`.
pub fn sample_run(id: &str, models: &[(&str, u32)]) -> RunResult {
    let now = Utc::now();
    RunResult {
        schema: RESULT_SCHEMA.into(),
        id: id.into(),
        created_at: now,
        finished_at: Some(now),
        status: RunStatus::Completed,
        app_version: APP_VERSION.into(),
        suite: SuiteRef {
            id: "quick".into(),
            name: "Quick Coding".into(),
            version: "1.0.0".into(),
            content_hash: "deadbeef".into(),
            official: true,
            task_count: 2,
        },
        settings: RunSettings::default(),
        standard_settings: true,
        system: sample_system(),
        providers: vec![],
        models: models
            .iter()
            .map(|(m, score)| {
                let tasks = vec![sample_task("t1", TaskStatus::Passed, 90.0), sample_task("t2", TaskStatus::Failed, 10.0)];
                ModelResult {
                    key: format!("ollama/{m}"),
                    model: sample_model(m),
                    status: ModelRunStatus::Completed,
                    error: None,
                    residency: None,
                    score: Some(ModelScore {
                        pulsebench_score: *score,
                        components: ComponentScores::default(),
                        categories: vec![],
                        formula: SCORE_FORMULA_VERSION.into(),
                    }),
                    stats: ModelStats {
                        tasks_total: 2,
                        tasks_passed: 1,
                        tasks_failed: 1,
                        pass_rate: 0.5,
                        avg_tokens_per_second: Some(42.0),
                        total_seconds: 10.0,
                        ..Default::default()
                    },
                    tasks,
                }
            })
            .collect(),
        log: vec![],
        telemetry: vec![],
        notes: vec![],
    }
}
