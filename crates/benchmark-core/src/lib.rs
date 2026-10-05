//! PulseBench benchmark engine: suites, prompts, the response protocol, verification and the runner.

pub mod checks;
pub mod diff;
pub mod error;
pub mod events;
pub mod prompt;
pub mod protocol;
pub mod report;
pub mod runner;
pub mod suite;
pub mod testparse;
pub mod validate;

pub use error::{CoreError, Result};
pub use events::{BroadcastSink, EventSink, NullSink, RunLog};
pub use runner::{model_key, run_benchmark, RunEnv, RunRequest};
pub use suite::{discover_suites, load_suite, write_lock, LoadedSuite, LoadedTask, LockStatus};
