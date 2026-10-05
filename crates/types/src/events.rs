//! Progress events streamed to the UI/CLI while a benchmark runs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::provider::GenerationProgress;
use crate::result::{LogEntry, RunStatus, TaskStatus};
use crate::system::TelemetrySample;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Phase {
    Preparing,
    LoadingModel,
    Generating,
    Parsing,
    Applying,
    Compiling,
    Testing,
    Verifying,
    Scoring,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum RunEvent {
    #[serde(rename_all = "camelCase")]
    RunStarted { run_id: String, suite_name: String, models: Vec<String>, tasks_per_model: u32 },
    #[serde(rename_all = "camelCase")]
    ModelStarted { run_id: String, model_key: String, display_name: String, index: u32, total: u32 },
    #[serde(rename_all = "camelCase")]
    TaskStarted { run_id: String, model_key: String, task_id: String, title: String, index: u32, total: u32 },
    #[serde(rename_all = "camelCase")]
    PhaseChanged { run_id: String, model_key: String, task_id: String, phase: Phase, attempt: u32 },
    #[serde(rename_all = "camelCase")]
    Generation { run_id: String, model_key: String, task_id: String, progress: GenerationProgress },
    #[serde(rename_all = "camelCase")]
    Telemetry { run_id: String, sample: TelemetrySample },
    #[serde(rename_all = "camelCase")]
    Log { run_id: String, entry: LogEntry },
    #[serde(rename_all = "camelCase")]
    TaskFinished { run_id: String, model_key: String, task_id: String, status: TaskStatus, score: Option<f64>, index: u32, total: u32 },
    #[serde(rename_all = "camelCase")]
    ModelFinished { run_id: String, model_key: String, score: Option<u32> },
    #[serde(rename_all = "camelCase")]
    RunFinished { run_id: String, status: RunStatus },
}
