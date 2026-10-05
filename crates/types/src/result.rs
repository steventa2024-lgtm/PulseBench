//! The public, versioned PulseBench result format (`pulsebench-result-v1`).

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::config::RunSettings;
use crate::provider::{GenerationResult, ModelInfo, ModelResidency, ProviderSnapshot};
use crate::suite::{Category, Difficulty, Language};
use crate::system::{ResourceSummary, SystemInfo, TelemetrySample};

pub const RESULT_SCHEMA: &str = "pulsebench-result-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum RunStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
    /// The app exited while the run was in progress; completed tasks were kept.
    Interrupted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ModelRunStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TaskStatus {
    /// Verification fully passed.
    Passed,
    /// The model produced a result that did not satisfy verification.
    Failed,
    /// The host could not run the task (missing runtime). Excluded from scoring.
    Skipped,
    /// Infrastructure/provider error. Scored as a failure and flagged.
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum FailureKind {
    ProviderError,
    GenerationTimeout,
    Truncated,
    ProtocolFailure,
    NoValidChanges,
    CompileFailed,
    TestsFailed,
    TestTimeout,
    NoTestsFound,
    MutantsSurvived,
    TooFewTests,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LogEntry {
    pub ts: DateTime<Utc>,
    pub level: LogLevel,
    /// Upper-case area tag: BENCHMARK, MODEL, TASK, GENERATION, PATCH, COMPILE, TEST, SCORE, ...
    pub scope: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SuiteRef {
    pub id: String,
    pub name: String,
    pub version: String,
    pub content_hash: String,
    pub official: bool,
    pub task_count: u32,
}

/// Per-component scores in `0.0..=1.0`. `None` = not applicable (e.g. no compile step).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComponentScores {
    pub correctness: Option<f64>,
    pub tests: Option<f64>,
    pub compile: Option<f64>,
    pub efficiency: Option<f64>,
    pub speed: Option<f64>,
    pub reliability: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskScore {
    /// `0.0..=100.0`
    pub value: f64,
    pub components: ComponentScores,
    /// Difficulty weight used when aggregating.
    pub weight: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CategoryScore {
    pub category: Category,
    /// `0.0..=100.0`
    pub score: f64,
    pub tasks: u32,
    pub passed: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelScore {
    /// `0..=10000`
    pub pulsebench_score: u32,
    pub components: ComponentScores,
    pub categories: Vec<CategoryScore>,
    pub formula: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelStats {
    pub tasks_total: u32,
    pub tasks_passed: u32,
    pub tasks_failed: u32,
    pub tasks_skipped: u32,
    pub tasks_error: u32,
    /// Passed / scored tasks, `0.0..=1.0`.
    pub pass_rate: f64,
    pub avg_generation_seconds: Option<f64>,
    /// Mean of per-attempt tokens/second.
    pub avg_tokens_per_second: Option<f64>,
    pub avg_time_to_first_token_ms: Option<f64>,
    #[ts(type = "number")]
    pub total_prompt_tokens: u64,
    #[ts(type = "number")]
    pub total_completion_tokens: u64,
    pub total_seconds: f64,
    pub repair_attempts: u32,
    pub compile_failures: u32,
    pub runtime_failures: u32,
    pub protocol_failures: u32,
    pub resources: ResourceSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandOutcome {
    pub command: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub duration_ms: u32,
    pub stdout: String,
    pub stderr: String,
    /// Output exceeded the capture limit and was cut.
    pub truncated: bool,
    /// The process could not be started (missing runtime, denied by policy, ...).
    pub spawn_error: Option<String>,
}

impl CommandOutcome {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out && !self.cancelled && self.spawn_error.is_none()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TestSummary {
    pub total: Option<u32>,
    pub passed: Option<u32>,
    pub failed: Option<u32>,
    pub skipped: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TestOutcome {
    pub command: CommandOutcome,
    pub summary: TestSummary,
    pub passed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MutantOutcome {
    pub id: String,
    pub description: String,
    /// The model's tests failed on this broken implementation (what we want).
    pub killed: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ParseMode {
    /// Clean protocol JSON.
    Json,
    /// Protocol JSON recovered from fences/prose, or files recovered from markdown code blocks.
    Recovered,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParseReport {
    pub mode: ParseMode,
    pub notes: Vec<String>,
    pub explanation: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ChangeStatus {
    Applied,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppliedChange {
    pub path: String,
    pub bytes: u32,
    pub status: ChangeStatus,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PromptRecord {
    pub system: String,
    pub user: String,
    /// Workspace-relative paths whose contents were included in the prompt.
    pub files: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Attempt {
    pub number: u32,
    pub prompt: PromptRecord,
    pub generation: Option<GenerationResult>,
    pub parse: Option<ParseReport>,
    pub changes: Vec<AppliedChange>,
    /// Unified diff of everything the model changed.
    pub diff: String,
    pub compile: Option<CommandOutcome>,
    pub tests: Option<TestOutcome>,
    pub mutants: Vec<MutantOutcome>,
    pub resources: ResourceSummary,
    pub failure: Option<FailureKind>,
    pub error: Option<String>,
    pub solved: bool,
    pub started_at: DateTime<Utc>,
    pub duration_ms: u32,
}

/// Outcome of running the untouched fixture, recorded to prove the task is not trivially passing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Baseline {
    pub compile: Option<CommandOutcome>,
    pub tests: Option<TestOutcome>,
    /// The unmodified fixture already satisfies verification (the task is suspect).
    pub already_passing: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskResult {
    pub id: String,
    pub title: String,
    pub category: Category,
    pub language: Language,
    pub difficulty: Difficulty,
    pub status: TaskStatus,
    pub failure: Option<FailureKind>,
    pub message: Option<String>,
    /// Attempts the task allowed (`maxAttempts` in task.json).
    pub max_attempts: u32,
    pub attempts: Vec<Attempt>,
    pub baseline: Option<Baseline>,
    pub score: Option<TaskScore>,
    pub started_at: DateTime<Utc>,
    pub duration_ms: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelResult {
    /// Stable key `providerId/modelId`.
    pub key: String,
    pub model: ModelInfo,
    pub status: ModelRunStatus,
    pub error: Option<String>,
    pub residency: Option<ModelResidency>,
    pub score: Option<ModelScore>,
    pub stats: ModelStats,
    pub tasks: Vec<TaskResult>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TelemetrySeries {
    pub model_key: String,
    pub samples: Vec<TelemetrySample>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunResult {
    /// Always `pulsebench-result-v1`.
    pub schema: String,
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: RunStatus,
    pub app_version: String,
    pub suite: SuiteRef,
    pub settings: RunSettings,
    /// Settings equal the standardized defaults (results are comparable with other standard runs).
    pub standard_settings: bool,
    pub system: SystemInfo,
    pub providers: Vec<ProviderSnapshot>,
    pub models: Vec<ModelResult>,
    pub log: Vec<LogEntry>,
    pub telemetry: Vec<TelemetrySeries>,
    /// Honest caveats attached to this run (execution mode, unavailable metrics, ...).
    pub notes: Vec<String>,
}

/// Lightweight row for history lists.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunSummary {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: RunStatus,
    pub suite_id: String,
    pub suite_name: String,
    pub suite_version: String,
    pub app_version: String,
    pub gpu: Option<String>,
    pub cpu: String,
    pub standard_settings: bool,
    pub models: Vec<ModelSummary>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelSummary {
    pub key: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
    pub status: ModelRunStatus,
    pub score: Option<u32>,
    pub pass_rate: f64,
    pub tasks_passed: u32,
    pub tasks_total: u32,
    pub avg_tokens_per_second: Option<f64>,
    pub total_seconds: f64,
    #[ts(type = "number | null")]
    pub peak_vram_mb: Option<u64>,
}

/// One point in a model's score history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryPoint {
    pub run_id: String,
    pub created_at: DateTime<Utc>,
    pub suite_id: String,
    pub suite_version: String,
    pub model_key: String,
    pub display_name: String,
    pub score: Option<u32>,
    pub pass_rate: f64,
    pub avg_tokens_per_second: Option<f64>,
    pub gpu: Option<String>,
    pub standard_settings: bool,
}
