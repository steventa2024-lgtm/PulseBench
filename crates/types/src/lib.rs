//! Shared data types for PulseBench.
//!
//! These types are the single source of truth for:
//! * the on-disk suite/task format,
//! * the public `pulsebench-result-v1` format,
//! * the Rust <-> TypeScript IPC surface (TypeScript is generated via `ts-rs`),
//! * the JSON Schema published in `packages/benchmark-schema` (generated via `schemars`).

pub mod config;
pub mod events;
pub mod provider;
pub mod result;
pub mod suite;
pub mod system;
#[doc(hidden)]
pub mod testing;

pub use config::*;
pub use events::*;
pub use provider::*;
pub use result::*;
pub use suite::*;
pub use system::*;

/// Version of the PulseBench application (workspace version).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// JSON Schema documents published with the repository.
pub mod schema {
    use schemars::schema_for;

    pub fn result_schema() -> serde_json::Value {
        let mut v = serde_json::to_value(schema_for!(super::RunResult)).expect("schema serializes");
        stamp(&mut v, "pulsebench-result-v1", "PulseBench run result");
        v
    }

    pub fn suite_schema() -> serde_json::Value {
        let mut v = serde_json::to_value(schema_for!(super::SuiteManifest)).expect("schema serializes");
        stamp(&mut v, "pulsebench-suite-v1", "PulseBench suite manifest (suite.json)");
        v
    }

    pub fn task_schema() -> serde_json::Value {
        let mut v = serde_json::to_value(schema_for!(super::Task)).expect("schema serializes");
        stamp(&mut v, "pulsebench-task-v1", "PulseBench task definition (task.json)");
        v
    }

    fn stamp(v: &mut serde_json::Value, id: &str, title: &str) {
        if let Some(o) = v.as_object_mut() {
            o.insert("$id".into(), format!("https://pulsebench.dev/schema/{id}.json").into());
            o.insert("title".into(), title.into());
        }
    }
}
