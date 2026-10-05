use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use futures_util::future::join_all;
use pulsebench_core::{discover_suites, load_suite, report, run_benchmark, BroadcastSink, EventSink, LoadedSuite, RunEnv, RunRequest};
use pulsebench_providers::{disabled_status, probe, ProviderRegistry};
use pulsebench_storage::Store;
use pulsebench_telemetry::{detect_system, Sampler};
use pulsebench_types::*;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, Result};
use crate::paths::AppPaths;
use crate::types::*;

const KEY_MASK: &str = "********";

struct Active {
    run_id: String,
    suite_id: String,
    models: Vec<String>,
    cancel: CancellationToken,
}

pub struct LaunchedRun {
    pub run_id: String,
    pub handle: JoinHandle<RunResult>,
}

pub struct App {
    pub paths: AppPaths,
    store: Arc<Store>,
    sink: Arc<BroadcastSink>,
    sampler: Sampler,
    system: RwLock<Option<SystemInfo>>,
    active: Mutex<Option<Active>>,
}

impl App {
    pub async fn new(paths: AppPaths) -> Result<Arc<App>> {
        std::fs::create_dir_all(&paths.data_dir)?;
        let store = Arc::new(Store::open(&paths.database())?);
        let interrupted = store.mark_interrupted()?;
        if interrupted > 0 {
            tracing::warn!(interrupted, "marked runs from a previous session as interrupted");
        }
        // Workspaces from a crashed session are garbage.
        let _ = std::fs::remove_dir_all(paths.work_dir());
        Ok(Arc::new(App {
            paths,
            store,
            sink: Arc::new(BroadcastSink::new(8192)),
            sampler: Sampler::start(std::time::Duration::from_secs(1)),
            system: RwLock::new(None),
            active: Mutex::new(None),
        }))
    }

    pub fn events(&self) -> broadcast::Receiver<RunEvent> {
        self.sink.subscribe()
    }

    pub fn data_info(&self) -> Result<DataInfo> {
        Ok(DataInfo {
            data_dir: self.paths.data_dir.display().to_string(),
            database_path: self.paths.database().display().to_string(),
            user_suites_dir: self.paths.user_suites().display().to_string(),
            bundled_suites_dirs: self.paths.bundled_suites.iter().map(|p| p.display().to_string()).collect(),
            app_version: APP_VERSION.into(),
            schema_version: self.store.schema_version()?,
        })
    }

    // ---- system ---------------------------------------------------------------------------

    pub async fn system_info(&self, refresh: bool) -> SystemInfo {
        if !refresh {
            if let Some(s) = self.system.read().await.clone() {
                return s;
            }
        }
        let info = detect_system().await;
        *self.system.write().await = Some(info.clone());
        info
    }

    pub async fn live_sample(&self) -> TelemetrySample {
        match self.sampler.latest() {
            Some(s) => s,
            None => self.sampler.sample_now().await,
        }
    }

    // ---- settings -------------------------------------------------------------------------

    /// Settings as shown to the UI: API keys are masked.
    pub fn settings(&self) -> Result<AppSettings> {
        let mut s = self.store.load_settings()?;
        for p in &mut s.providers {
            if p.api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false) {
                p.api_key = Some(KEY_MASK.into());
            }
        }
        Ok(s)
    }

    pub fn save_settings(&self, mut new: AppSettings) -> Result<AppSettings> {
        validate_settings(&new)?;
        let old = self.store.load_settings()?;
        for p in &mut new.providers {
            if p.api_key.as_deref() == Some(KEY_MASK) {
                p.api_key = old.providers.iter().find(|o| o.id == p.id).and_then(|o| o.api_key.clone());
            }
            if p.api_key.as_deref() == Some("") {
                p.api_key = None;
            }
            p.base_url = p.base_url.trim().trim_end_matches('/').to_string();
        }
        self.store.save_settings(&new)?;
        self.settings()
    }

    pub fn mark_onboarding_done(&self) -> Result<()> {
        let mut s = self.store.load_settings()?;
        s.onboarding_done = true;
        self.store.save_settings(&s)?;
        Ok(())
    }

    // ---- providers ------------------------------------------------------------------------

    pub fn registry(&self) -> Result<ProviderRegistry> {
        Ok(ProviderRegistry::from_configs(&self.store.load_settings()?.providers))
    }

    /// Probe every configured provider concurrently. Never fails: problems are part of the status.
    pub async fn provider_statuses(&self) -> Result<Vec<ProviderStatus>> {
        let settings = self.store.load_settings()?;
        let futs = settings.providers.iter().map(|cfg| async move {
            if !cfg.enabled {
                return disabled_status(cfg);
            }
            let p = pulsebench_providers::build_provider(cfg);
            probe(p.as_ref()).await
        });
        Ok(join_all(futs).await)
    }

    // ---- suites ---------------------------------------------------------------------------

    fn suite_roots(&self) -> Result<Vec<PathBuf>> {
        let mut roots = self.paths.bundled_suites.clone();
        roots.push(self.paths.user_suites());
        roots.extend(self.store.load_settings()?.suite_dirs.iter().map(PathBuf::from));
        Ok(roots)
    }

    pub fn load_suites(&self) -> Result<(Vec<LoadedSuite>, Vec<SuiteProblem>)> {
        let (suites, errors) = discover_suites(&self.suite_roots()?);
        let problems = errors.into_iter().map(|(p, e)| SuiteProblem { path: p.display().to_string(), error: e.to_string() }).collect();
        Ok((suites, problems))
    }

    pub fn suites(&self) -> Result<SuiteList> {
        let (suites, problems) = self.load_suites()?;
        Ok(SuiteList { suites: suites.iter().map(|s| s.summary()).collect(), problems })
    }

    pub fn find_suite(&self, id: &str) -> Result<Arc<LoadedSuite>> {
        let (suites, _) = self.load_suites()?;
        suites.into_iter().find(|s| s.manifest.id == id).map(Arc::new).ok_or_else(|| AppError::NotFound(format!("suite '{id}' not found")))
    }

    /// Validate a suite folder and copy it into the user's suites directory.
    pub fn import_suite(&self, folder: &Path) -> Result<pulsebench_types::SuiteSummary> {
        let suite = load_suite(folder)?;
        let dest = self.paths.user_suites().join(&suite.manifest.id);
        if dest.exists() {
            return Err(AppError::Invalid(format!("a custom suite with id '{}' is already imported", suite.manifest.id)));
        }
        if self.find_suite(&suite.manifest.id).is_ok() {
            return Err(AppError::Invalid(format!("a suite with id '{}' already exists; change its id first", suite.manifest.id)));
        }
        copy_dir(folder, &dest)?;
        Ok(load_suite(&dest)?.summary())
    }

    pub fn remove_custom_suite(&self, id: &str) -> Result<()> {
        let dir = self.paths.user_suites().join(id);
        if !dir.join("suite.json").is_file() {
            return Err(AppError::NotFound(format!("custom suite '{id}' not found (bundled suites cannot be removed)")));
        }
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }

    /// Install the toolchain a suite needs (one-time download of pinned npm packages).
    pub async fn prepare_suite(&self, suite_id: &str, cancel: &CancellationToken) -> Result<PrepareReport> {
        let suite = self.find_suite(suite_id)?;
        let Some(spec) = &suite.manifest.toolchain.node else {
            return Ok(PrepareReport { suite_id: suite_id.into(), ready: true, message: "This suite needs no extra toolchain.".into() });
        };
        let sink = self.sink.clone();
        let id = suite_id.to_string();
        let res = pulsebench_sandbox::toolchain::prepare_node_toolchain(
            &self.paths.toolchain_dir(),
            spec,
            suite.node_lockfile.as_deref(),
            cancel,
            &move |m| {
                sink.emit(RunEvent::Log {
                    run_id: format!("prepare-{id}"),
                    entry: LogEntry { ts: Utc::now(), level: LogLevel::Info, scope: "TOOLCHAIN".into(), message: m },
                })
            },
        )
        .await;
        Ok(match res {
            Ok(dir) => PrepareReport { suite_id: suite_id.into(), ready: true, message: format!("Toolchain ready at {}", dir.display()) },
            Err(e) => PrepareReport { suite_id: suite_id.into(), ready: false, message: e.to_string() },
        })
    }

    // ---- runs -----------------------------------------------------------------------------

    pub fn active_run(&self) -> Option<ActiveRunInfo> {
        self.active.lock().unwrap().as_ref().map(|a| ActiveRunInfo {
            run_id: a.run_id.clone(),
            suite_id: a.suite_id.clone(),
            models: a.models.clone(),
            status: ModelRunStatus::Running,
        })
    }

    pub fn cancel_run(&self) -> bool {
        match self.active.lock().unwrap().as_ref() {
            Some(a) => {
                a.cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// Start a run in the background and return immediately.
    pub async fn start_run(self: &Arc<Self>, req: StartRunRequest) -> Result<String> {
        Ok(self.launch_run(req).await?.run_id)
    }

    /// Like [`App::start_run`] but also returns the join handle (used by the CLI).
    pub async fn launch_run(self: &Arc<Self>, req: StartRunRequest) -> Result<LaunchedRun> {
        if let Some(a) = self.active_run() {
            return Err(AppError::RunInProgress(a.run_id));
        }
        if req.models.is_empty() {
            return Err(AppError::Invalid("select at least one model".into()));
        }
        let settings = self.store.load_settings()?;
        let suite = self.find_suite(&req.suite_id)?;
        if let Some(ids) = &req.task_ids {
            for id in ids {
                if suite.task(id).is_none() {
                    return Err(AppError::Invalid(format!("suite '{}' has no task '{id}'", suite.manifest.id)));
                }
            }
        }
        let run_settings = req.settings.clone().unwrap_or_else(|| settings.run.clone());
        validate_run_settings(&run_settings)?;

        // Resolve every selected model against its live provider.
        let registry = ProviderRegistry::from_configs(&settings.providers);
        let mut models = Vec::new();
        for sel in &req.models {
            let provider = registry
                .get(&sel.provider_id)
                .ok_or_else(|| AppError::Invalid(format!("provider '{}' is not enabled", sel.provider_id)))?;
            let info = provider
                .model_info(&sel.model_id)
                .await
                .map_err(|e| AppError::Invalid(format!("{}/{}: {e}", sel.provider_id, sel.model_id)))?;
            if models.iter().any(|m: &ModelInfo| m.provider_id == info.provider_id && m.id == info.id) {
                return Err(AppError::Invalid(format!("model {} selected twice", info.id)));
            }
            models.push(info);
        }

        let run_id = format!("{}-{}", Utc::now().format("%Y%m%d-%H%M%S"), &uuid::Uuid::new_v4().simple().to_string()[..6]);
        let cancel = CancellationToken::new();
        {
            let mut a = self.active.lock().unwrap();
            if let Some(existing) = a.as_ref() {
                return Err(AppError::RunInProgress(existing.run_id.clone()));
            }
            *a = Some(Active {
                run_id: run_id.clone(),
                suite_id: suite.manifest.id.clone(),
                models: models.iter().map(|m| m.display_name.clone()).collect(),
                cancel: cancel.clone(),
            });
        }

        let system = self.system_info(false).await;
        let store = self.store.clone();
        let store_cp = store.clone();
        let env = RunEnv {
            providers: registry,
            work_root: self.paths.work_dir(),
            toolchain_root: self.paths.toolchain_dir(),
            sampler: Some(self.sampler.clone()),
            system,
            sink: self.sink.clone(),
            checkpoint: Some(Arc::new(move |run: &RunResult| {
                if let Err(e) = store_cp.save_run(run, false) {
                    tracing::error!("could not checkpoint run {}: {e}", run.id);
                }
            })),
        };
        let request = RunRequest {
            run_id: run_id.clone(),
            suite,
            models,
            task_ids: req.task_ids.clone(),
            settings: run_settings,
            keep_workspaces: settings.keep_workspaces,
        };
        let me = self.clone();
        let rid = run_id.clone();
        let handle = tokio::spawn(async move {
            let run = run_benchmark(&env, request, cancel).await;
            if let Err(e) = store.save_run(&run, true) {
                tracing::error!("could not save run {}: {e}", rid);
            }
            *me.active.lock().unwrap() = None;
            run
        });
        Ok(LaunchedRun { run_id, handle })
    }

    /// Cancel any active run and wait briefly for it to wind down (application shutdown).
    pub async fn shutdown(&self) {
        if self.cancel_run() {
            for _ in 0..100 {
                if self.active_run().is_none() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
        self.sampler.stop();
    }

    // ---- history & export -----------------------------------------------------------------

    pub fn list_runs(&self, limit: u32) -> Result<Vec<RunSummary>> {
        Ok(self.store.list_runs(limit)?)
    }

    pub fn get_run(&self, id: &str) -> Result<RunResult> {
        self.store.get_run(id)?.ok_or_else(|| AppError::NotFound(format!("run '{id}' not found")))
    }

    pub fn delete_run(&self, id: &str) -> Result<()> {
        if self.active_run().map(|a| a.run_id == id).unwrap_or(false) {
            return Err(AppError::Invalid("cannot delete a run that is in progress".into()));
        }
        self.store.delete_run(id)?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<usize> {
        if self.active_run().is_some() {
            return Err(AppError::Invalid("cannot clear history while a benchmark is running".into()));
        }
        Ok(self.store.clear_history()?)
    }

    pub fn model_history(&self, model_key: &str, suite_id: Option<&str>) -> Result<Vec<HistoryPoint>> {
        Ok(self.store.model_history(model_key, suite_id)?)
    }

    pub fn latest_scores(&self) -> Result<Vec<LatestScore>> {
        Ok(self
            .store
            .latest_scores()?
            .into_iter()
            .map(|(model_key, score, suite_name)| LatestScore { model_key, score, suite_name })
            .collect())
    }

    pub fn export_run(&self, id: &str, format: ExportFormat, model_key: Option<&str>) -> Result<String> {
        let run = self.get_run(id)?;
        match format {
            ExportFormat::Json => Ok(serde_json::to_string_pretty(&run).map_err(|e| AppError::Invalid(e.to_string()))? + "\n"),
            ExportFormat::Markdown => Ok(report::markdown_report(&run)),
            ExportFormat::Svg => {
                let key = match model_key {
                    Some(k) => k.to_string(),
                    None => {
                        report::ranked(&run).first().map(|m| m.key.clone()).ok_or_else(|| AppError::Invalid("run has no models".into()))?
                    }
                };
                report::share_card_svg(&run, &key).ok_or_else(|| AppError::Invalid(format!("no score available for model '{key}'")))
            }
        }
    }

    pub fn export_run_to_file(&self, id: &str, format: ExportFormat, model_key: Option<&str>, dest: &Path) -> Result<()> {
        let text = self.export_run(id, format, model_key)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(dest, text)?;
        Ok(())
    }

    pub fn export_database(&self, dest: &Path) -> Result<()> {
        Ok(self.store.export_to(dest)?)
    }
}

fn validate_run_settings(r: &RunSettings) -> Result<()> {
    let g = &r.generation;
    if !(0.0..=2.0).contains(&g.temperature) {
        return Err(AppError::Invalid("temperature must be between 0 and 2".into()));
    }
    if !(512..=1_048_576).contains(&g.context_tokens) {
        return Err(AppError::Invalid("context limit must be between 512 and 1,048,576 tokens".into()));
    }
    if !(64..=262_144).contains(&g.max_output_tokens) {
        return Err(AppError::Invalid("generation limit must be between 64 and 262,144 tokens".into()));
    }
    if !(5..=7200).contains(&g.timeout_seconds) {
        return Err(AppError::Invalid("generation timeout must be between 5 and 7200 seconds".into()));
    }
    let w = &r.scoring.weights;
    if [w.correctness, w.tests, w.compile, w.efficiency, w.speed, w.reliability].iter().any(|x| *x < 0.0 || !x.is_finite())
        || w.correctness + w.tests + w.compile + w.efficiency + w.speed + w.reliability <= 0.0
    {
        return Err(AppError::Invalid("score weights must be non-negative and not all zero".into()));
    }
    Ok(())
}

fn validate_settings(s: &AppSettings) -> Result<()> {
    validate_run_settings(&s.run)?;
    let mut ids = std::collections::HashSet::new();
    for p in &s.providers {
        if p.id.is_empty() || !p.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(AppError::Invalid(format!("provider id '{}' must be alphanumeric, '-' or '_'", p.id)));
        }
        if !ids.insert(p.id.clone()) {
            return Err(AppError::Invalid(format!("duplicate provider id '{}'", p.id)));
        }
        let url = p.base_url.trim();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(AppError::Invalid(format!("{}: URL must start with http:// or https://", p.name)));
        }
    }
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let ty = e.file_type()?;
        let to = dst.join(e.file_name());
        let name = e.file_name().to_string_lossy().into_owned();
        if ty.is_symlink() || name == "node_modules" || name == "__pycache__" || name == ".git" {
            continue;
        }
        if ty.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), &to)?;
        }
    }
    Ok(())
}
