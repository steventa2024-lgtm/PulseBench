//! Benchmark suite and task definitions (the on-disk, versioned benchmark format).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Current schema version of `suite.json` / `task.json`.
pub const SUITE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Category {
    Bugfix,
    Implementation,
    Refactoring,
    Testing,
    Security,
    Reasoning,
}

impl Category {
    pub const ALL: [Category; 6] =
        [Category::Bugfix, Category::Implementation, Category::Refactoring, Category::Testing, Category::Security, Category::Reasoning];

    pub fn label(self) -> &'static str {
        match self {
            Category::Bugfix => "Bug fixing",
            Category::Implementation => "Implementation",
            Category::Refactoring => "Refactoring",
            Category::Testing => "Testing",
            Category::Security => "Security",
            Category::Reasoning => "Reasoning",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Language {
    Python,
    Javascript,
    Typescript,
    /// TypeScript + JSX (React function components).
    React,
}

impl Language {
    /// Logical runtime the language needs on the host (or in the Docker image).
    pub fn runtime(self) -> Runtime {
        match self {
            Language::Python => Runtime::Python,
            _ => Runtime::Node,
        }
    }

    /// Fenced-code language tag used in prompts.
    pub fn fence_tag(self) -> &'static str {
        match self {
            Language::Python => "python",
            Language::Javascript => "javascript",
            Language::Typescript => "typescript",
            Language::React => "tsx",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Runtime {
    Python,
    Node,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

/// Extra verification applied after the normal test command passes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Verification {
    /// The task's test command must exit 0 and report at least one test.
    #[default]
    Tests,
    /// For "write tests" tasks: the model's tests must pass on the correct
    /// implementation and must fail on every mutant (a broken implementation).
    #[serde(rename_all = "camelCase")]
    Mutation {
        /// Minimum number of tests the model's suite must contain.
        #[serde(default)]
        min_tests: u32,
        mutants: Vec<Mutant>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Mutant {
    pub id: String,
    pub description: String,
    /// workspace-relative target path -> path (relative to the task directory) of the mutated file.
    pub overlay: BTreeMap<String, String>,
}

/// One benchmark task (`tasks/<id>/task.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub category: Category,
    pub language: Language,
    pub difficulty: Difficulty,
    pub description: String,
    /// Fixture directory, relative to the task directory.
    #[serde(default = "default_workspace")]
    pub workspace: String,
    /// Files shown to the model. Also editable unless `editableFiles` is set.
    pub entry_files: Vec<String>,
    /// Files the model may create or overwrite. Defaults to `entryFiles`.
    #[serde(default)]
    pub editable_files: Option<Vec<String>>,
    /// Read-only files shown to the model as context (for example the tests).
    #[serde(default)]
    pub context_files: Vec<String>,
    /// Trusted compile/type-check command. Never supplied by the model.
    #[serde(default)]
    pub compile_command: Option<String>,
    /// Trusted test command. Never supplied by the model.
    pub test_command: String,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u32,
    #[serde(default = "default_attempts")]
    pub max_attempts: u32,
    #[serde(default)]
    pub verification: Verification,
}

fn default_workspace() -> String {
    "workspace".into()
}
fn default_timeout() -> u32 {
    120
}
fn default_attempts() -> u32 {
    1
}

impl Task {
    pub fn editable(&self) -> &[String] {
        self.editable_files.as_deref().unwrap_or(&self.entry_files)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NodeToolchain {
    /// Exact-pinned npm packages installed once into a shared, read-only toolchain cache.
    pub packages: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolchainSpec {
    #[serde(default)]
    pub node: Option<NodeToolchain>,
}

/// `suite.json`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SuiteManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    /// Bump whenever any task, fixture, test or scoring-relevant file changes.
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub official: bool,
    /// Task directory names under `tasks/`, in execution order.
    pub tasks: Vec<String>,
    #[serde(default)]
    pub toolchain: ToolchainSpec,
    /// Rough wall-clock estimate shown in the UI.
    #[serde(default)]
    pub estimated_minutes: Option<String>,
}

/// Summary of an installed suite for list views.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SuiteSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub official: bool,
    pub task_count: u32,
    pub categories: BTreeMap<String, u32>,
    pub languages: BTreeMap<String, u32>,
    pub content_hash: String,
    /// `Some(true)` when an official suite's lock file matches its content.
    pub lock_verified: Option<bool>,
    pub estimated_minutes: Option<String>,
    pub path: String,
    pub tasks: Vec<TaskSummary>,
    pub requires: Vec<Runtime>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub category: Category,
    pub language: Language,
    pub difficulty: Difficulty,
}
