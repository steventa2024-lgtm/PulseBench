//! PulseBench application service.
//!
//! The desktop shell (Tauri) and the CLI are both thin wrappers around [`App`]; there is no
//! benchmark logic outside this crate and `pulsebench-core`.

mod error;
mod paths;
pub mod rpc;
mod service;
mod types;

pub use error::{AppError, Result};
pub use paths::AppPaths;
pub use service::{App, LaunchedRun};
pub use types::*;
