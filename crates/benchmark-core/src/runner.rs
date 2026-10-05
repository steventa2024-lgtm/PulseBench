//! The benchmark engine. The same code path serves the desktop app and the CLI.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use pulsebench_providers::{snapshot, InferenceProvider, ProviderError, ProviderRegistry};
use pulsebench_sandbox::{toolchain, Sandbox, Workspace};
use pulsebench_scoring as scoring;
use pulsebench_telemetry::Sampler;
use pulsebench_types::*;
use tokio_util::sync::CancellationToken;

use crate::checks::{run_checks, CheckOutcome};
use crate::diff::unified_diff;
use crate::events::{EventSink, RunLog};
use crate::prompt::{build_user_prompt, PromptFile, RepairContext, SYSTEM_PROMPT};
use crate::protocol::{parse_response, validate_changes};
use crate::suite::{LoadedSuite, LoadedTask};

/// Models that error this many tasks in a row are abandoned (the provider is evidently broken).
const MAX_CONSECUTIVE_ERRORS: u32 = 3;

/// Called with the partial result after every finished task.
pub type CheckpointFn = Arc<dyn Fn(&RunResult) + Send + Sync>;

pub struct RunEnv {
    pub providers: ProviderRegistry,
    /// Parent directory for disposable workspaces.
    pub work_root: PathBuf,
    /// Cache directory for prepared toolchains.
    pub toolchain_root: PathBuf,
    pub sampler: Option<Sampler>,
    pub system: SystemInfo,
    pub sink: Arc<dyn EventSink>,
    /// Called with the partial result after every finished task (crash-safe history).
    pub checkpoint: Option<CheckpointFn>,
}

pub struct RunRequest {
    pub run_id: String,
    pub suite: Arc<LoadedSuite>,
    pub models: Vec<ModelInfo>,
    /// Restrict the run to these task ids (CLI `--task`); `None` runs the whole suite.
    pub task_ids: Option<Vec<String>>,
    pub settings: RunSettings,
    pub keep_workspaces: bool,
}

pub fn model_key(m: &ModelInfo) -> String {
    format!("{}/{}", m.provider_id, m.id)
}

/// Run the benchmark. Never panics on provider/runtime problems: they are recorded in the result.
pub async fn run_benchmark(env: &RunEnv, req: RunRequest, cancel: CancellationToken) -> RunResult {
    let log = Arc::new(RunLog::new(&req.run_id, env.sink.clone()));
    let suite = req.suite.clone();
    let tasks: Vec<&LoadedTask> =
        suite.tasks.iter().filter(|t| req.task_ids.as_ref().map(|ids| ids.contains(&t.task.id)).unwrap_or(true)).collect();

    // --- result skeleton ------------------------------------------------------------------
    let mut providers = Vec::new();
    for id in req.models.iter().map(|m| m.provider_id.clone()).collect::<std::collections::BTreeSet<_>>() {
        if let Some(p) = env.providers.get(&id) {
            providers.push(snapshot(p.as_ref()).await);
        }
    }
    let mut run = RunResult {
        schema: RESULT_SCHEMA.into(),
        id: req.run_id.clone(),
        created_at: Utc::now(),
        finished_at: None,
        status: RunStatus::Running,
        app_version: APP_VERSION.into(),
        suite: SuiteRef {
            id: suite.manifest.id.clone(),
            name: suite.manifest.name.clone(),
            version: suite.manifest.version.clone(),
            content_hash: suite.content_hash.clone(),
            official: suite.is_verified_official(),
            task_count: tasks.len() as u32,
        },
        standard_settings: req.settings.is_standard() && req.task_ids.is_none(),
        settings: req.settings.clone(),
        system: env.system.clone(),
        providers,
        models: req
            .models
            .iter()
            .map(|m| ModelResult {
                key: model_key(m),
                model: m.clone(),
                status: ModelRunStatus::Pending,
                error: None,
                residency: None,
                score: None,
                stats: ModelStats::default(),
                tasks: vec![],
            })
            .collect(),
        log: vec![],
        telemetry: vec![],
        notes: vec![],
    };

    // --- environment preparation ----------------------------------------------------------
    let mut node_toolchain: Option<PathBuf> = None;
    let mut unavailable: HashMap<Runtime, String> = HashMap::new();
    let needed: Vec<Runtime> = {
        let mut v: Vec<Runtime> = tasks.iter().map(|t| t.task.language.runtime()).collect();
        v.sort_by_key(|r| *r as u8);
        v.dedup();
        v
    };
    match req.settings.execution {
        ExecutionMode::Local => {
            for rt in &needed {
                let name = pulsebench_sandbox::runtime::runtime_program(*rt);
                if pulsebench_sandbox::runtime::resolve_program(name).await.is_none() {
                    unavailable.insert(*rt, format!("{name} was not found on this machine"));
                }
            }
        }
        ExecutionMode::Docker => {
            let d = pulsebench_sandbox::runtime::detect_docker().await;
            if !d.running {
                for rt in &needed {
                    unavailable.insert(*rt, "Docker execution is selected but the Docker daemon is not reachable".into());
                }
            }
        }
    }
    if needed.contains(&Runtime::Node) && !unavailable.contains_key(&Runtime::Node) {
        if let Some(spec) = &suite.manifest.toolchain.node {
            let lg = log.clone();
            let res = toolchain::prepare_node_toolchain(&env.toolchain_root, spec, suite.node_lockfile.as_deref(), &cancel, &move |m| {
                lg.info("TOOLCHAIN", m)
            })
            .await;
            match res {
                Ok(dir) => node_toolchain = Some(dir),
                Err(e) => {
                    log.error("TOOLCHAIN", format!("Node toolchain unavailable: {e}"));
                    unavailable.insert(Runtime::Node, format!("Node toolchain unavailable: {e}"));
                }
            }
        }
    }
    let sandbox = Sandbox::new(req.settings.execution, req.settings.docker_images.clone(), env.work_root.join(&req.run_id), node_toolchain);
    run.notes.push(format!("Execution: {}", sandbox.describe()));
    if req.settings.generation.temperature == 0.0 && req.settings.generation.seed.is_some() {
        run.notes.push("Greedy decoding was requested (temperature 0 with a fixed seed). Providers do not guarantee bit-identical output across hardware, drivers or versions.".into());
    } else {
        run.notes.push("Non-deterministic sampling was requested; results will vary between runs.".into());
    }
    if !run.standard_settings {
        run.notes.push(
            "Non-standard settings or a task subset were used; scores are not directly comparable with default-configuration runs.".into(),
        );
    }
    if !suite.is_verified_official() && suite.manifest.official {
        run.notes.push("This suite claims to be official but does not match its lock file; it was modified without a version bump.".into());
    }
    for p in &run.providers {
        if !p.capabilities.honors_context_length {
            run.notes.push(format!(
                "{}: the context length setting cannot be applied through this API; the model's load-time context is used.",
                p.name
            ));
        }
    }
    if env.system.gpus.iter().all(|g| !g.telemetry_available) {
        run.notes.push(
            "GPU utilization/VRAM telemetry is unavailable on this machine (no nvidia-smi); those metrics are reported as unavailable."
                .into(),
        );
    }

    log.info(
        "BENCHMARK",
        format!("started: {} v{} ({} tasks, {} models)", suite.manifest.name, suite.manifest.version, tasks.len(), req.models.len()),
    );
    env.sink.emit(RunEvent::RunStarted {
        run_id: req.run_id.clone(),
        suite_name: suite.manifest.name.clone(),
        models: req.models.iter().map(|m| m.display_name.clone()).collect(),
        tasks_per_model: tasks.len() as u32,
    });

    // Forward live telemetry to the UI.
    let forwarder = env.sampler.as_ref().map(|s| {
        let mut rx = s.subscribe();
        let sink = env.sink.clone();
        let run_id = req.run_id.clone();
        tokio::spawn(async move {
            while let Ok(sample) = rx.recv().await {
                sink.emit(RunEvent::Telemetry { run_id: run_id.clone(), sample });
            }
        })
    });

    let baselines: Arc<Mutex<HashMap<String, Baseline>>> = Arc::default();
    let ctx = TaskCtx {
        env,
        sandbox: &sandbox,
        settings: &req.settings,
        log: log.clone(),
        run_id: &req.run_id,
        keep_workspaces: req.keep_workspaces,
        baselines,
        cancel: &cancel,
        unavailable: &unavailable,
    };

    // --- per-model loop --------------------------------------------------------------------
    let total_models = req.models.len() as u32;
    for (mi, model) in req.models.iter().enumerate() {
        if cancel.is_cancelled() {
            break;
        }
        let key = model_key(model);
        let model_started = Utc::now();
        run.models[mi].status = ModelRunStatus::Running;
        env.sink.emit(RunEvent::ModelStarted {
            run_id: req.run_id.clone(),
            model_key: key.clone(),
            display_name: model.display_name.clone(),
            index: mi as u32,
            total: total_models,
        });
        log.info("MODEL", format!("{} ({}/{})", model.id, mi + 1, total_models));

        let Some(provider) = env.providers.get(&model.provider_id) else {
            run.models[mi].status = ModelRunStatus::Failed;
            run.models[mi].error = Some(format!("provider '{}' is not configured or is disabled", model.provider_id));
            log.error("MODEL", run.models[mi].error.clone().unwrap());
            continue;
        };

        // Load the model with the run's settings so the first task does not pay for loading.
        ctx.phase(&key, "-", Phase::LoadingModel, 0);
        log.info("MODEL", format!("loading {} into memory…", model.id));
        match warm_up(provider.as_ref(), &model.id, &req.settings.generation, &cancel).await {
            Ok(()) => {}
            Err(ProviderError::Cancelled) => {
                run.models[mi].status = ModelRunStatus::Cancelled;
                break;
            }
            Err(e) => {
                run.models[mi].status = ModelRunStatus::Failed;
                run.models[mi].error = Some(format!("could not load model: {e}"));
                log.error("MODEL", format!("{}: could not load: {e}", model.id));
                env.sink.emit(RunEvent::ModelFinished { run_id: req.run_id.clone(), model_key: key.clone(), score: None });
                continue;
            }
        }
        run.models[mi].residency = provider.residency(&model.id).await.ok().flatten();

        let mut consecutive_errors = 0u32;
        let mut cancelled_here = false;
        for (ti, lt) in tasks.iter().enumerate() {
            if cancel.is_cancelled() {
                cancelled_here = true;
                break;
            }
            match ctx.run_task(provider.as_ref(), model, &key, lt, ti as u32, tasks.len() as u32).await {
                Some(result) => {
                    consecutive_errors = if result.status == TaskStatus::Error { consecutive_errors + 1 } else { 0 };
                    run.models[mi].tasks.push(result);
                    refresh_model(&mut run.models[mi], &req.settings.scoring, None);
                    run.log = log.entries();
                    if let Some(cp) = &env.checkpoint {
                        cp(&run);
                    }
                }
                None => {
                    cancelled_here = true;
                    break;
                }
            }
            if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                run.models[mi].error = Some(format!("abandoned after {MAX_CONSECUTIVE_ERRORS} consecutive provider errors"));
                log.error("MODEL", format!("{}: {}", model.id, run.models[mi].error.clone().unwrap()));
                break;
            }
        }

        // Model-level resources and telemetry window.
        let model_ended = Utc::now();
        let resources = match &env.sampler {
            Some(s) => {
                let samples = s.samples_between(model_started, model_ended);
                run.telemetry.push(TelemetrySeries { model_key: key.clone(), samples: samples.clone() });
                pulsebench_telemetry::summarize(&samples)
            }
            None => ResourceSummary::default(),
        };
        refresh_model(&mut run.models[mi], &req.settings.scoring, Some(resources));
        run.models[mi].status = if cancelled_here {
            ModelRunStatus::Cancelled
        } else if run.models[mi].error.is_some() {
            ModelRunStatus::Failed
        } else {
            ModelRunStatus::Completed
        };
        let _ = provider.unload(&model.id).await;
        let score = run.models[mi].score.as_ref().map(|s| s.pulsebench_score);
        if let Some(s) = score {
            log.info("SCORE", format!("{} = {s}", model.id));
        }
        env.sink.emit(RunEvent::ModelFinished { run_id: req.run_id.clone(), model_key: key.clone(), score });
        if cancelled_here {
            break;
        }
    }

    // --- finish ------------------------------------------------------------------------------
    if let Some(f) = forwarder {
        f.abort();
    }
    for m in run.models.iter_mut().filter(|m| m.status == ModelRunStatus::Pending || m.status == ModelRunStatus::Running) {
        m.status = ModelRunStatus::Cancelled;
    }
    run.status = if cancel.is_cancelled() {
        RunStatus::Cancelled
    } else if run.models.iter().all(|m| m.status == ModelRunStatus::Failed) {
        RunStatus::Failed
    } else {
        RunStatus::Completed
    };
    run.finished_at = Some(Utc::now());
    log.info(
        "BENCHMARK",
        match run.status {
            RunStatus::Cancelled => "cancelled (completed task results were kept)",
            RunStatus::Failed => "failed",
            _ => "finished",
        },
    );
    run.log = log.entries();
    let _ = std::fs::remove_dir_all(env.work_root.join(&req.run_id));
    env.sink.emit(RunEvent::RunFinished { run_id: req.run_id.clone(), status: run.status });
    run
}

async fn warm_up(
    p: &dyn InferenceProvider,
    model: &str,
    settings: &GenerationSettings,
    cancel: &CancellationToken,
) -> Result<(), ProviderError> {
    tokio::select! {
        _ = cancel.cancelled() => Err(ProviderError::Cancelled),
        r = p.warm_up(model, settings) => r,
    }
}

/// Recompute stats and score from the tasks finished so far.
fn refresh_model(m: &mut ModelResult, cfg: &ScoringConfig, resources: Option<ResourceSummary>) {
    let resources = resources.unwrap_or_else(|| m.stats.resources.clone());
    m.stats = scoring::model_stats(&m.tasks, resources);
    m.score = scoring::score_model(&m.tasks, cfg);
}

struct TaskCtx<'a> {
    env: &'a RunEnv,
    sandbox: &'a Sandbox,
    settings: &'a RunSettings,
    log: Arc<RunLog>,
    run_id: &'a str,
    keep_workspaces: bool,
    baselines: Arc<Mutex<HashMap<String, Baseline>>>,
    cancel: &'a CancellationToken,
    unavailable: &'a HashMap<Runtime, String>,
}

impl TaskCtx<'_> {
    fn phase(&self, model_key: &str, task_id: &str, phase: Phase, attempt: u32) {
        self.env.sink.emit(RunEvent::PhaseChanged {
            run_id: self.run_id.to_string(),
            model_key: model_key.to_string(),
            task_id: task_id.to_string(),
            phase,
            attempt,
        });
    }

    /// `None` means the run was cancelled while this task was in flight (the task is dropped).
    async fn run_task(
        &self,
        provider: &dyn InferenceProvider,
        model: &ModelInfo,
        key: &str,
        lt: &LoadedTask,
        index: u32,
        total: u32,
    ) -> Option<TaskResult> {
        let task = &lt.task;
        let started_at = Utc::now();
        let t0 = Instant::now();
        self.env.sink.emit(RunEvent::TaskStarted {
            run_id: self.run_id.to_string(),
            model_key: key.to_string(),
            task_id: task.id.clone(),
            title: task.title.clone(),
            index,
            total,
        });
        self.log.info("TASK", format!("{} started ({}/{})", task.id, index + 1, total));

        let mut result = TaskResult {
            id: task.id.clone(),
            title: task.title.clone(),
            category: task.category,
            language: task.language,
            difficulty: task.difficulty,
            status: TaskStatus::Failed,
            failure: None,
            message: None,
            max_attempts: task.max_attempts,
            attempts: vec![],
            baseline: None,
            score: None,
            started_at,
            duration_ms: 0,
        };

        if let Some(reason) = self.unavailable.get(&task.language.runtime()) {
            result.status = TaskStatus::Skipped;
            result.message = Some(reason.clone());
            self.log.warn("TASK", format!("{} skipped: {reason}", task.id));
            return Some(self.finish(result, key, index, total, t0));
        }

        let sandbox = self.sandbox.clone();
        let (fixture, tid) = (lt.fixture_dir(), task.id.clone());
        let ws = match tokio::task::spawn_blocking(move || sandbox.create_workspace(&tid, &fixture)).await {
            Ok(Ok(ws)) => ws,
            other => {
                let msg = match other {
                    Ok(Err(e)) => e.to_string(),
                    Err(e) => e.to_string(),
                    Ok(Ok(_)) => unreachable!(),
                };
                result.status = TaskStatus::Error;
                result.message = Some(format!("could not create workspace: {msg}"));
                self.log.error("TASK", format!("{}: {}", task.id, result.message.clone().unwrap()));
                return Some(self.finish(result, key, index, total, t0));
            }
        };

        let outcome = self.run_task_in_workspace(provider, model, key, lt, &ws, &mut result).await;
        if !self.keep_workspaces {
            let _ = tokio::task::spawn_blocking(move || ws.destroy()).await;
        } else {
            self.log.info("TASK", format!("workspace kept for inspection: {}", ws.root().display()));
        }
        if outcome == Flow::Cancelled {
            self.log.warn("TASK", format!("{} cancelled", task.id));
            return None;
        }
        Some(self.finish(result, key, index, total, t0))
    }

    fn finish(&self, mut result: TaskResult, key: &str, index: u32, total: u32, t0: Instant) -> TaskResult {
        result.duration_ms = t0.elapsed().as_millis().min(u32::MAX as u128) as u32;
        result.score = scoring::score_task(&result, &self.settings.scoring);
        let value = result.score.as_ref().map(|s| s.value);
        match result.status {
            TaskStatus::Passed => self.log.info("TEST", format!("{} PASS", result.id)),
            TaskStatus::Failed => {
                self.log.warn("TEST", format!("{} FAIL ({})", result.id, result.failure.map(|f| format!("{f:?}")).unwrap_or_default()))
            }
            TaskStatus::Error => self.log.error("TASK", format!("{} ERROR: {}", result.id, result.message.clone().unwrap_or_default())),
            TaskStatus::Skipped => {}
        }
        if let Some(v) = value {
            self.log.info("SCORE", format!("{} {:.1}", result.id, v));
        }
        self.env.sink.emit(RunEvent::TaskFinished {
            run_id: self.run_id.to_string(),
            model_key: key.to_string(),
            task_id: result.id.clone(),
            status: result.status,
            score: value,
            index,
            total,
        });
        result
    }

    async fn run_task_in_workspace(
        &self,
        provider: &dyn InferenceProvider,
        model: &ModelInfo,
        key: &str,
        lt: &LoadedTask,
        ws: &Workspace,
        result: &mut TaskResult,
    ) -> Flow {
        let task = &lt.task;
        let phase_cb = |p: Phase| self.phase(key, &task.id, p, 0);

        // Baseline: identical for every model, so computed once per run.
        let cached = self.baselines.lock().unwrap().get(&task.id).cloned();
        let baseline = match cached {
            Some(b) => b,
            None => {
                self.log.info("BASELINE", format!("{}: running the unmodified fixture", task.id));
                let checks = run_checks(self.sandbox, ws, lt, false, self.cancel, &phase_cb).await;
                if checks.cancelled() || self.cancel.is_cancelled() {
                    return Flow::Cancelled;
                }
                let already_passing = checks.solved() && matches!(task.verification, Verification::Tests);
                if already_passing {
                    self.log.warn("BASELINE", format!("{}: the unmodified fixture already passes — the task is suspect", task.id));
                }
                let b = Baseline { compile: checks.compile, tests: checks.tests, already_passing };
                self.baselines.lock().unwrap().insert(task.id.clone(), b.clone());
                b
            }
        };
        result.baseline = Some(baseline);

        // Original contents for diffs and prompts.
        let editable: Vec<String> = task.editable().to_vec();
        let mut originals: BTreeMap<String, Option<String>> = BTreeMap::new();
        for p in &editable {
            originals.insert(p.clone(), ws.read_text(p).ok().flatten());
        }
        let mut applied: BTreeMap<String, String> = BTreeMap::new();
        let mut repair: Option<(String, String)> = None; // (failure, output)

        for attempt_no in 1..=task.max_attempts {
            if self.cancel.is_cancelled() {
                return Flow::Cancelled;
            }
            let a_start = Utc::now();
            let a_t0 = Instant::now();
            if let Some(s) = &self.env.sampler {
                s.sample_now().await;
            }

            // Prompt (identical text for every model on attempt 1).
            let modifiable: Vec<PromptFile> =
                editable.iter().map(|p| PromptFile { path: p.clone(), content: ws.read_text(p).ok().flatten() }).collect();
            let context: Vec<PromptFile> =
                task.context_files.iter().map(|p| PromptFile { path: p.clone(), content: ws.read_text(p).ok().flatten() }).collect();
            let repair_ctx = repair.as_ref().map(|(f, o)| RepairContext { attempt: attempt_no - 1, failure: f, output: o });
            let user = build_user_prompt(task, &modifiable, &context, repair_ctx.as_ref());
            let mut attempt = Attempt {
                number: attempt_no,
                prompt: PromptRecord {
                    system: SYSTEM_PROMPT.to_string(),
                    user: user.clone(),
                    files: modifiable.iter().chain(context.iter()).map(|f| f.path.clone()).collect(),
                },
                generation: None,
                parse: None,
                changes: vec![],
                diff: String::new(),
                compile: None,
                tests: None,
                mutants: vec![],
                resources: ResourceSummary::default(),
                failure: None,
                error: None,
                solved: false,
                started_at: a_start,
                duration_ms: 0,
            };

            // Generate.
            self.phase(key, &task.id, Phase::Generating, attempt_no);
            self.log.info("GENERATION", format!("{} attempt {}/{} → {}", task.id, attempt_no, task.max_attempts, model.id));
            let request = GenerationRequest {
                model: model.id.clone(),
                system: SYSTEM_PROMPT.to_string(),
                prompt: user,
                settings: self.settings.generation.clone(),
            };
            let last_emit = Mutex::new(Instant::now() - Duration::from_secs(1));
            let (sink, run_id, mk, tid) = (self.env.sink.clone(), self.run_id.to_string(), key.to_string(), task.id.clone());
            let progress = move |p: GenerationProgress| {
                let mut last = last_emit.lock().unwrap();
                if last.elapsed() >= Duration::from_millis(150) {
                    *last = Instant::now();
                    sink.emit(RunEvent::Generation { run_id: run_id.clone(), model_key: mk.clone(), task_id: tid.clone(), progress: p });
                }
            };
            let gen = provider.generate(&request, self.cancel, &progress).await;
            let generation = match gen {
                Ok(g) => g,
                Err(ProviderError::Cancelled) => return Flow::Cancelled,
                Err(e) => {
                    let timeout = e.is_timeout();
                    attempt.failure = Some(if timeout { FailureKind::GenerationTimeout } else { FailureKind::ProviderError });
                    attempt.error = Some(e.to_string());
                    attempt.duration_ms = a_t0.elapsed().as_millis() as u32;
                    result.status = if timeout { TaskStatus::Failed } else { TaskStatus::Error };
                    result.failure = attempt.failure;
                    result.message = attempt.error.clone();
                    self.log.error("GENERATION", format!("{}: {e}", task.id));
                    result.attempts.push(attempt);
                    return Flow::Done;
                }
            };
            self.log.info(
                "GENERATION",
                format!(
                    "complete ({:.1}s, {} tokens, {})",
                    generation.duration_ms as f64 / 1000.0,
                    generation.completion_tokens.map(|n| n.to_string()).unwrap_or_else(|| "? ".into()),
                    generation.tokens_per_second.map(|t| format!("{t:.1} tok/s")).unwrap_or_else(|| "tok/s unavailable".into())
                ),
            );
            let truncated = generation.finish_reason == Some(FinishReason::Length);

            // Parse + validate.
            self.phase(key, &task.id, Phase::Parsing, attempt_no);
            let parsed = parse_response(&generation.text, &editable);
            attempt.generation = Some(generation);
            attempt.parse = Some(parsed.report.clone());
            let validated = validate_changes(parsed.changes, &editable);
            attempt.changes = validated.records;
            if parsed.report.mode == ParseMode::Failed || validated.accepted.is_empty() {
                attempt.failure = Some(if truncated {
                    FailureKind::Truncated
                } else if parsed.report.mode == ParseMode::Failed {
                    FailureKind::ProtocolFailure
                } else {
                    FailureKind::NoValidChanges
                });
                let why = attempt.parse.as_ref().map(|p| p.notes.join("; ")).unwrap_or_default();
                let rejected: Vec<String> =
                    attempt.changes.iter().filter_map(|c| c.reason.as_ref().map(|r| format!("{}: {r}", c.path))).collect();
                self.log.warn(
                    "PATCH",
                    format!(
                        "{}: no usable changes ({why}{})",
                        task.id,
                        if rejected.is_empty() { String::new() } else { format!("; {}", rejected.join("; ")) }
                    ),
                );
                attempt.duration_ms = a_t0.elapsed().as_millis() as u32;
                repair = Some(("your previous response could not be used".into(), format!("{}\n{}", why, rejected.join("\n"))));
                self.finalize_attempt(&mut attempt, a_start);
                result.failure = attempt.failure;
                result.attempts.push(attempt);
                continue;
            }
            if truncated {
                self.log.warn("GENERATION", format!("{}: output hit the token limit but was parseable", task.id));
            }

            // Apply inside the disposable workspace only.
            self.phase(key, &task.id, Phase::Applying, attempt_no);
            for c in &validated.accepted {
                if let Err(e) = ws.write_text(&c.path, &c.content) {
                    // Path validation already passed; a write failure is an infrastructure fault.
                    attempt.failure = Some(FailureKind::ProviderError);
                    attempt.error = Some(format!("could not write {}: {e}", c.path));
                    result.status = TaskStatus::Error;
                    result.message = attempt.error.clone();
                    attempt.duration_ms = a_t0.elapsed().as_millis() as u32;
                    result.attempts.push(attempt);
                    return Flow::Done;
                }
                applied.insert(c.path.clone(), c.content.clone());
                self.log.info("PATCH", format!("applied {}", c.path));
            }
            attempt.diff = applied
                .iter()
                .map(|(p, new)| unified_diff(p, originals.get(p).and_then(|o| o.as_deref()), new))
                .filter(|d| !d.is_empty())
                .collect::<Vec<_>>()
                .join("\n");

            // Compile, test, verify.
            let phase_cb = |p: Phase| self.phase(key, &task.id, p, attempt_no);
            self.log.info("TEST", format!("{}: {}", task.id, task.test_command));
            let checks: CheckOutcome = run_checks(self.sandbox, ws, lt, true, self.cancel, &phase_cb).await;
            if checks.cancelled() || self.cancel.is_cancelled() {
                return Flow::Cancelled;
            }
            if let Some(spawn) =
                checks.compile.iter().map(|c| &c.spawn_error).chain(checks.tests.iter().map(|t| &t.command.spawn_error)).flatten().next()
            {
                // The *benchmark* could not run its own command: infrastructure problem, not the model's fault.
                attempt.failure = Some(FailureKind::ProviderError);
                attempt.error = Some(format!("could not run benchmark command: {spawn}"));
                attempt.compile = checks.compile;
                attempt.tests = checks.tests;
                result.status = TaskStatus::Error;
                result.message = attempt.error.clone();
                attempt.duration_ms = a_t0.elapsed().as_millis() as u32;
                self.finalize_attempt(&mut attempt, a_start);
                result.attempts.push(attempt);
                return Flow::Done;
            }
            attempt.compile = checks.compile.clone();
            attempt.tests = checks.tests.clone();
            attempt.mutants = checks.mutants.clone();
            attempt.failure = checks.failure;
            attempt.solved = checks.solved();
            attempt.duration_ms = a_t0.elapsed().as_millis() as u32;
            self.finalize_attempt(&mut attempt, a_start);

            if let Some(t) = &attempt.tests {
                let s = &t.summary;
                self.log.info(
                    "TEST",
                    format!(
                        "{} {}{}",
                        task.id,
                        if attempt.solved { "PASS" } else { "FAIL" },
                        match (s.passed, s.total) {
                            (Some(p), Some(t)) => format!(" ({p}/{t} tests)"),
                            _ => String::new(),
                        }
                    ),
                );
            }
            result.failure = attempt.failure;
            let solved = attempt.solved;
            if !solved {
                let (why, out) = checks.failure_feedback();
                repair = Some((why, out));
            }
            result.attempts.push(attempt);
            if solved {
                result.status = TaskStatus::Passed;
                result.failure = None;
                return Flow::Done;
            }
        }
        result.status = TaskStatus::Failed;
        Flow::Done
    }

    fn finalize_attempt(&self, attempt: &mut Attempt, started: chrono::DateTime<Utc>) {
        if let Some(s) = &self.env.sampler {
            attempt.resources = s.summarize(started, Utc::now());
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Flow {
    Done,
    Cancelled,
}
