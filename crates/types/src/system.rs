//! Hardware description and telemetry samples. Missing values are `None`, never guessed.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: GpuVendor,
    #[ts(type = "number | null")]
    pub vram_total_mb: Option<u64>,
    #[ts(type = "number | null")]
    pub vram_used_mb: Option<u64>,
    pub driver_version: Option<String>,
    /// `true` when utilization/VRAM/temperature can be sampled live (nvidia-smi).
    pub telemetry_available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub hostname_hash: String,
    pub cpu_model: String,
    pub cpu_physical_cores: Option<u32>,
    pub cpu_logical_cores: u32,
    #[ts(type = "number")]
    pub ram_total_mb: u64,
    #[ts(type = "number")]
    pub ram_available_mb: u64,
    pub gpus: Vec<GpuInfo>,
    /// Detected container runtime, if any.
    pub docker: DockerStatus,
    pub runtimes: Vec<RuntimeStatus>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DockerStatus {
    pub installed: bool,
    /// Daemon reachable (`docker info` succeeded).
    pub running: bool,
    pub version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimeStatus {
    pub name: String,
    pub available: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TelemetrySample {
    pub ts: DateTime<Utc>,
    /// Whole-system CPU utilization, percent.
    pub cpu_percent: Option<f32>,
    #[ts(type = "number")]
    pub ram_used_mb: u64,
    #[ts(type = "number")]
    pub ram_total_mb: u64,
    pub gpu_percent: Option<f32>,
    #[ts(type = "number | null")]
    pub vram_used_mb: Option<u64>,
    #[ts(type = "number | null")]
    pub vram_total_mb: Option<u64>,
    pub gpu_temp_c: Option<f32>,
}

/// Peak/average resource figures over a time window (a task or a whole model).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResourceSummary {
    pub samples: u32,
    pub peak_cpu_percent: Option<f32>,
    pub avg_cpu_percent: Option<f32>,
    #[ts(type = "number | null")]
    pub peak_ram_mb: Option<u64>,
    pub peak_gpu_percent: Option<f32>,
    #[ts(type = "number | null")]
    pub peak_vram_mb: Option<u64>,
    pub peak_gpu_temp_c: Option<f32>,
}
