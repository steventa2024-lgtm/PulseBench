//! Run configuration, scoring configuration and provider configuration.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const SCORE_FORMULA_VERSION: &str = "pulsebench-score-v1";

/// Generation settings applied identically to every model in a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct GenerationSettings {
    pub temperature: f32,
    pub seed: Option<u32>,
    pub context_tokens: u32,
    pub max_output_tokens: u32,
    /// Wall-clock limit for a single generation call.
    pub timeout_seconds: u32,
}

impl Default for GenerationSettings {
    fn default() -> Self {
        Self { temperature: 0.0, seed: Some(42), context_tokens: 16384, max_output_tokens: 8192, timeout_seconds: 600 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ExecutionMode {
    /// Disposable temp directory on the host with a sanitized environment.
    /// NOT an OS-level security boundary.
    Local,
    /// `docker run --network none` with the workspace bind-mounted.
    Docker,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct DockerImages {
    pub python: String,
    pub node: String,
}

impl Default for DockerImages {
    fn default() -> Self {
        Self { python: "python:3.12-slim".into(), node: "node:22-slim".into() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ScoreWeights {
    pub correctness: f64,
    pub tests: f64,
    pub compile: f64,
    pub efficiency: f64,
    pub speed: f64,
    pub reliability: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self { correctness: 60.0, tests: 20.0, compile: 5.0, efficiency: 5.0, speed: 5.0, reliability: 5.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct DifficultyWeights {
    pub easy: f64,
    pub medium: f64,
    pub hard: f64,
}

impl Default for DifficultyWeights {
    fn default() -> Self {
        Self { easy: 1.0, medium: 2.0, hard: 3.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ScoringConfig {
    pub formula: String,
    pub weights: ScoreWeights,
    pub difficulty_weights: DifficultyWeights,
    /// Generation speed at or below this maps to a speed component of 0.
    pub speed_floor_tps: f64,
    /// Generation speed at or above this maps to a speed component of 1.
    pub speed_ceiling_tps: f64,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            formula: SCORE_FORMULA_VERSION.into(),
            weights: ScoreWeights::default(),
            difficulty_weights: DifficultyWeights::default(),
            speed_floor_tps: 5.0,
            speed_ceiling_tps: 100.0,
        }
    }
}

/// Everything that influences a run; stored with the result for reproducibility.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct RunSettings {
    pub generation: GenerationSettings,
    pub execution: ExecutionMode,
    pub docker_images: DockerImages,
    pub scoring: ScoringConfig,
    /// Fixed system prompt text shared by all models (recorded verbatim in results).
    pub system_prompt_version: String,
}

impl Default for RunSettings {
    fn default() -> Self {
        Self {
            generation: GenerationSettings::default(),
            execution: ExecutionMode::Local,
            docker_images: DockerImages::default(),
            scoring: ScoringConfig::default(),
            system_prompt_version: "pulsebench-prompt-v1".into(),
        }
    }
}

impl RunSettings {
    /// True when every setting equals the standardized default, i.e. results are
    /// directly comparable with other default-configuration runs.
    pub fn is_standard(&self) -> bool {
        let d = RunSettings::default();
        self.generation == d.generation && self.scoring == d.scoring && self.docker_images == d.docker_images
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ProviderKind {
    Ollama,
    OpenaiCompatible,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProviderConfig {
    pub id: String,
    pub kind: ProviderKind,
    pub name: String,
    pub base_url: String,
    pub enabled: bool,
    /// Never written to results or exports. Redacted when sent to the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

impl ProviderConfig {
    pub fn defaults() -> Vec<ProviderConfig> {
        vec![
            ProviderConfig {
                id: "ollama".into(),
                kind: ProviderKind::Ollama,
                name: "Ollama".into(),
                base_url: "http://localhost:11434".into(),
                enabled: true,
                api_key: None,
            },
            ProviderConfig {
                id: "lmstudio".into(),
                kind: ProviderKind::OpenaiCompatible,
                name: "LM Studio".into(),
                base_url: "http://localhost:1234/v1".into(),
                enabled: true,
                api_key: None,
            },
        ]
    }
}

/// Persisted application settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AppSettings {
    pub providers: Vec<ProviderConfig>,
    pub run: RunSettings,
    pub keep_workspaces: bool,
    pub onboarding_done: bool,
    /// Extra directories scanned for custom suites.
    pub suite_dirs: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            providers: ProviderConfig::defaults(),
            run: RunSettings::default(),
            keep_workspaces: false,
            onboarding_done: false,
            suite_dirs: vec![],
        }
    }
}
