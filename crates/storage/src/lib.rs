//! SQLite persistence: settings, providers, benchmark history.
//!
//! `benchmark_runs.result_json` holds the authoritative `pulsebench-result-v1` document; the other
//! tables are denormalized indexes for listing, comparing and querying history.

use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use pulsebench_types::{
    AppSettings, HistoryPoint, ModelRunStatus, ModelSummary, ProviderConfig, ProviderKind, RunResult, RunStatus, RunSummary,
};
use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not (de)serialize stored data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// Ordered migrations. Never edit an applied migration; append a new one.
const MIGRATIONS: &[(i64, &str, &str)] =
    &[(1, "init", include_str!("../migrations/0001_init.sql")), (2, "indexes", include_str!("../migrations/0002_indexes.sql"))];

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| StorageError::Other(format!("cannot create {}: {e}", parent.display())))?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Store { conn: Mutex::new(conn) };
        store.migrate_to(i64::MAX)?;
        Ok(store)
    }

    /// Apply migrations up to and including `max_version` (exposed for upgrade tests).
    pub fn migrate_to(&self, max_version: i64) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL);",
        )?;
        let current: i64 = conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))?;
        for (version, name, sql) in MIGRATIONS.iter().filter(|(v, _, _)| *v > current && *v <= max_version) {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![version, name, Utc::now().to_rfc3339()],
            )?;
            tx.commit()?;
            tracing::info!(version, name, "applied migration");
        }
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))?)
    }

    // ---- settings -----------------------------------------------------------------------

    pub fn load_settings(&self) -> Result<AppSettings> {
        let conn = self.conn.lock().unwrap();
        let mut settings: AppSettings =
            match conn.query_row("SELECT value_json FROM settings WHERE key = 'app'", [], |r| r.get::<_, String>(0)).optional()? {
                Some(json) => serde_json::from_str(&json)?,
                None => AppSettings::default(),
            };
        let mut stmt = conn.prepare("SELECT id, kind, name, base_url, enabled, api_key FROM providers ORDER BY position")?;
        let providers: Vec<ProviderConfig> = stmt
            .query_map([], |r| {
                let kind: String = r.get(1)?;
                Ok(ProviderConfig {
                    id: r.get(0)?,
                    kind: if kind == "ollama" { ProviderKind::Ollama } else { ProviderKind::OpenaiCompatible },
                    name: r.get(2)?,
                    base_url: r.get(3)?,
                    enabled: r.get::<_, i64>(4)? != 0,
                    api_key: r.get(5)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        if !providers.is_empty() {
            settings.providers = providers;
        }
        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        // Providers live in their own table; the rest is a JSON document.
        let mut rest = settings.clone();
        rest.providers = vec![];
        tx.execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES ('app', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![serde_json::to_string(&rest)?, Utc::now().to_rfc3339()],
        )?;
        tx.execute("DELETE FROM providers", [])?;
        for (i, p) in settings.providers.iter().enumerate() {
            tx.execute(
                "INSERT INTO providers (id, kind, name, base_url, enabled, api_key, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    p.id,
                    match p.kind {
                        ProviderKind::Ollama => "ollama",
                        ProviderKind::OpenaiCompatible => "openai-compatible",
                    },
                    p.name,
                    p.base_url,
                    p.enabled as i64,
                    p.api_key,
                    i as i64
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // ---- runs ---------------------------------------------------------------------------

    /// Persist a run. With `finalize == false` only the run document is stored (cheap checkpoint
    /// after every task); with `true` the query tables are rebuilt as well.
    pub fn save_run(&self, run: &RunResult, finalize: bool) -> Result<()> {
        let json = serde_json::to_string(run)?;
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO benchmark_runs
               (id, created_at, finished_at, status, suite_id, suite_name, suite_version, suite_hash, app_version,
                os, cpu, gpu, ram_mb, standard_settings, schema, config_json, result_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
             ON CONFLICT(id) DO UPDATE SET
               finished_at = excluded.finished_at, status = excluded.status, result_json = excluded.result_json,
               config_json = excluded.config_json",
            params![
                run.id,
                run.created_at.to_rfc3339(),
                run.finished_at.map(|d| d.to_rfc3339()),
                status_str(run.status),
                run.suite.id,
                run.suite.name,
                run.suite.version,
                run.suite.content_hash,
                run.app_version,
                format!("{} {}", run.system.os_name, run.system.os_version).trim().to_string(),
                run.system.cpu_model,
                run.system.gpus.first().map(|g| g.name.clone()),
                run.system.ram_total_mb as i64,
                run.standard_settings as i64,
                run.schema,
                serde_json::to_string(&run.settings)?,
                json,
            ],
        )?;
        if finalize {
            tx.execute("DELETE FROM benchmark_models WHERE run_id = ?1", [&run.id])?;
            tx.execute("DELETE FROM task_results WHERE run_id = ?1", [&run.id])?;
            tx.execute("DELETE FROM telemetry_samples WHERE run_id = ?1", [&run.id])?;
            for m in &run.models {
                tx.execute(
                    "INSERT INTO benchmark_models
                       (run_id, model_key, provider_id, model_id, display_name, status, score, pass_rate, tasks_passed,
                        tasks_total, avg_tps, peak_vram_mb, total_seconds, meta_json)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                    params![
                        run.id,
                        m.key,
                        m.model.provider_id,
                        m.model.id,
                        m.model.display_name,
                        model_status_str(m.status),
                        m.score.as_ref().map(|s| s.pulsebench_score as i64),
                        m.stats.pass_rate,
                        m.stats.tasks_passed as i64,
                        m.stats.tasks_total as i64,
                        m.stats.avg_tokens_per_second,
                        m.stats.resources.peak_vram_mb.map(|v| v as i64),
                        m.stats.total_seconds,
                        serde_json::to_string(&m.model)?,
                    ],
                )?;
                for t in &m.tasks {
                    tx.execute(
                        "INSERT OR IGNORE INTO benchmark_tasks (suite_id, suite_version, task_id, title, category, language, difficulty)
                         VALUES (?1,?2,?3,?4,?5,?6,?7)",
                        params![
                            run.suite.id,
                            run.suite.version,
                            t.id,
                            t.title,
                            enum_str(&t.category)?,
                            enum_str(&t.language)?,
                            enum_str(&t.difficulty)?
                        ],
                    )?;
                    let gen: Vec<_> = t.attempts.iter().filter_map(|a| a.generation.as_ref()).collect();
                    tx.execute(
                        "INSERT INTO task_results
                           (run_id, model_key, task_id, status, failure, score, attempts, duration_ms, gen_ms,
                            completion_tokens, tokens_per_second, result_json)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                        params![
                            run.id,
                            m.key,
                            t.id,
                            enum_str(&t.status)?,
                            t.failure.as_ref().map(enum_str).transpose()?,
                            t.score.as_ref().map(|s| s.value),
                            t.attempts.len() as i64,
                            t.duration_ms as i64,
                            (!gen.is_empty()).then(|| gen.iter().map(|g| g.duration_ms as i64).sum::<i64>()),
                            (!gen.is_empty()).then(|| gen.iter().map(|g| g.completion_tokens.unwrap_or(0) as i64).sum::<i64>()),
                            gen.iter().rev().find_map(|g| g.tokens_per_second),
                            serde_json::to_string(t)?,
                        ],
                    )?;
                }
            }
            for series in &run.telemetry {
                let mut stmt = tx.prepare_cached(
                    "INSERT INTO telemetry_samples (run_id, model_key, ts_ms, cpu_percent, ram_used_mb, gpu_percent, vram_used_mb, gpu_temp_c)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                )?;
                for s in &series.samples {
                    stmt.execute(params![
                        run.id,
                        series.model_key,
                        s.ts.timestamp_millis(),
                        s.cpu_percent,
                        s.ram_used_mb as i64,
                        s.gpu_percent,
                        s.vram_used_mb.map(|v| v as i64),
                        s.gpu_temp_c
                    ])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_run(&self, id: &str) -> Result<Option<RunResult>> {
        let conn = self.conn.lock().unwrap();
        let json: Option<String> = conn.query_row("SELECT result_json FROM benchmark_runs WHERE id = ?1", [id], |r| r.get(0)).optional()?;
        Ok(json.map(|j| serde_json::from_str(&j)).transpose()?)
    }

    pub fn delete_run(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.execute("DELETE FROM benchmark_runs WHERE id = ?1", [id])? > 0)
    }

    pub fn clear_history(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.execute("DELETE FROM benchmark_runs", [])?)
    }

    /// Runs that were still marked `running` when the app last exited.
    pub fn mark_interrupted(&self) -> Result<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let ids: Vec<String> = {
            let mut stmt = tx.prepare("SELECT id FROM benchmark_runs WHERE status = 'running'")?;
            let rows = stmt.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
            rows
        };
        for id in &ids {
            let json: String = tx.query_row("SELECT result_json FROM benchmark_runs WHERE id = ?1", [id], |r| r.get(0))?;
            let mut run: RunResult = serde_json::from_str(&json)?;
            run.status = RunStatus::Interrupted;
            for m in &mut run.models {
                if matches!(m.status, ModelRunStatus::Running | ModelRunStatus::Pending) {
                    m.status = ModelRunStatus::Cancelled;
                }
            }
            tx.execute(
                "UPDATE benchmark_runs SET status = 'interrupted', result_json = ?2 WHERE id = ?1",
                params![id, serde_json::to_string(&run)?],
            )?;
        }
        tx.commit()?;
        Ok(ids.len())
    }

    pub fn list_runs(&self, limit: u32) -> Result<Vec<RunSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, created_at, finished_at, status, suite_id, suite_name, suite_version, app_version, gpu, cpu, standard_settings
             FROM benchmark_runs ORDER BY created_at DESC LIMIT ?1",
        )?;
        let mut runs: Vec<RunSummary> = stmt
            .query_map([limit], |r| {
                Ok(RunSummary {
                    id: r.get(0)?,
                    created_at: parse_ts(&r.get::<_, String>(1)?),
                    finished_at: r.get::<_, Option<String>>(2)?.map(|s| parse_ts(&s)),
                    status: parse_status(&r.get::<_, String>(3)?),
                    suite_id: r.get(4)?,
                    suite_name: r.get(5)?,
                    suite_version: r.get(6)?,
                    app_version: r.get(7)?,
                    gpu: r.get(8)?,
                    cpu: r.get(9)?,
                    standard_settings: r.get::<_, i64>(10)? != 0,
                    models: vec![],
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        let mut mstmt = conn.prepare(
            "SELECT model_key, provider_id, model_id, display_name, status, score, pass_rate, tasks_passed, tasks_total,
                    avg_tps, total_seconds, peak_vram_mb
             FROM benchmark_models WHERE run_id = ?1 ORDER BY score DESC NULLS LAST, display_name",
        )?;
        for run in &mut runs {
            run.models = mstmt
                .query_map([&run.id], |r| {
                    Ok(ModelSummary {
                        key: r.get(0)?,
                        provider_id: r.get(1)?,
                        model_id: r.get(2)?,
                        display_name: r.get(3)?,
                        status: parse_model_status(&r.get::<_, String>(4)?),
                        score: r.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                        pass_rate: r.get(6)?,
                        tasks_passed: r.get::<_, i64>(7)? as u32,
                        tasks_total: r.get::<_, i64>(8)? as u32,
                        avg_tokens_per_second: r.get(9)?,
                        total_seconds: r.get(10)?,
                        peak_vram_mb: r.get::<_, Option<i64>>(11)?.map(|v| v as u64),
                    })
                })?
                .collect::<std::result::Result<_, _>>()?;
        }
        Ok(runs)
    }

    /// Score history for one model (oldest first), optionally restricted to a suite.
    pub fn model_history(&self, model_key: &str, suite_id: Option<&str>) -> Result<Vec<HistoryPoint>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT r.id, r.created_at, r.suite_id, r.suite_version, m.model_key, m.display_name, m.score, m.pass_rate,
                    m.avg_tps, r.gpu, r.standard_settings
             FROM benchmark_models m JOIN benchmark_runs r ON r.id = m.run_id
             WHERE m.model_key = ?1 AND (?2 IS NULL OR r.suite_id = ?2) AND m.score IS NOT NULL
             ORDER BY r.created_at ASC",
        )?;
        let rows = stmt
            .query_map(params![model_key, suite_id], |r| {
                Ok(HistoryPoint {
                    run_id: r.get(0)?,
                    created_at: parse_ts(&r.get::<_, String>(1)?),
                    suite_id: r.get(2)?,
                    suite_version: r.get(3)?,
                    model_key: r.get(4)?,
                    display_name: r.get(5)?,
                    score: r.get::<_, Option<i64>>(6)?.map(|v| v as u32),
                    pass_rate: r.get(7)?,
                    avg_tokens_per_second: r.get(8)?,
                    gpu: r.get(9)?,
                    standard_settings: r.get::<_, i64>(10)? != 0,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Latest score per model key across all runs (for dashboard model cards).
    pub fn latest_scores(&self) -> Result<Vec<(String, u32, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT m.model_key, m.score, r.suite_name
             FROM benchmark_models m JOIN benchmark_runs r ON r.id = m.run_id
             WHERE m.score IS NOT NULL
             ORDER BY r.created_at DESC",
        )?;
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u32, r.get::<_, String>(2)?)))? {
            let (k, s, n) = row?;
            if seen.insert(k.clone()) {
                out.push((k, s, n));
            }
        }
        Ok(out)
    }

    /// Write a consistent copy of the database to `dest` (`VACUUM INTO`).
    pub fn export_to(&self, dest: &Path) -> Result<()> {
        if dest.exists() {
            return Err(StorageError::Other(format!("{} already exists", dest.display())));
        }
        let conn = self.conn.lock().unwrap();
        conn.execute("VACUUM INTO ?1", [dest.to_string_lossy().as_ref()])?;
        Ok(())
    }
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now())
}

fn enum_str<T: serde::Serialize>(v: &T) -> Result<String> {
    Ok(serde_json::to_value(v)?.as_str().unwrap_or_default().to_string())
}

fn status_str(s: RunStatus) -> &'static str {
    match s {
        RunStatus::Running => "running",
        RunStatus::Completed => "completed",
        RunStatus::Cancelled => "cancelled",
        RunStatus::Failed => "failed",
        RunStatus::Interrupted => "interrupted",
    }
}

fn parse_status(s: &str) -> RunStatus {
    match s {
        "running" => RunStatus::Running,
        "completed" => RunStatus::Completed,
        "cancelled" => RunStatus::Cancelled,
        "interrupted" => RunStatus::Interrupted,
        _ => RunStatus::Failed,
    }
}

fn model_status_str(s: ModelRunStatus) -> &'static str {
    match s {
        ModelRunStatus::Pending => "pending",
        ModelRunStatus::Running => "running",
        ModelRunStatus::Completed => "completed",
        ModelRunStatus::Cancelled => "cancelled",
        ModelRunStatus::Failed => "failed",
    }
}

fn parse_model_status(s: &str) -> ModelRunStatus {
    match s {
        "pending" => ModelRunStatus::Pending,
        "running" => ModelRunStatus::Running,
        "completed" => ModelRunStatus::Completed,
        "cancelled" => ModelRunStatus::Cancelled,
        _ => ModelRunStatus::Failed,
    }
}

#[cfg(test)]
mod tests;
