use pulsebench_types::{ModelRunStatus, RunSettings};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelSelection {
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StartRunRequest {
    pub suite_id: String,
    pub models: Vec<ModelSelection>,
    /// Run only these tasks (makes the run non-standard).
    #[serde(default)]
    pub task_ids: Option<Vec<String>>,
    /// Overrides the saved run settings for this run only.
    #[serde(default)]
    pub settings: Option<RunSettings>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SuiteProblem {
    pub path: String,
    pub error: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SuiteList {
    pub suites: Vec<pulsebench_types::SuiteSummary>,
    pub problems: Vec<SuiteProblem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ExportFormat {
    Json,
    Markdown,
    /// Share card for one model (needs `modelKey`).
    Svg,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Json => "json",
            ExportFormat::Markdown => "md",
            ExportFormat::Svg => "svg",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LatestScore {
    pub model_key: String,
    pub score: u32,
    pub suite_name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActiveRunInfo {
    pub run_id: String,
    pub suite_id: String,
    pub models: Vec<String>,
    pub status: ModelRunStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DataInfo {
    pub data_dir: String,
    pub database_path: String,
    pub user_suites_dir: String,
    pub bundled_suites_dirs: Vec<String>,
    pub app_version: String,
    #[ts(type = "number")]
    pub schema_version: i64,
}

/// What `prepare_suite` reports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrepareReport {
    pub suite_id: String,
    pub ready: bool,
    pub message: String,
}
