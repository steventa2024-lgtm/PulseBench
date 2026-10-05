//! Event sink and the run log.

use std::sync::{Arc, Mutex};

use chrono::Utc;
use pulsebench_types::{LogEntry, LogLevel, RunEvent};
use tokio::sync::broadcast;

pub trait EventSink: Send + Sync {
    fn emit(&self, event: RunEvent);
}

pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _event: RunEvent) {}
}

/// Fan-out to any number of subscribers (UI, CLI printer, tests).
#[derive(Clone)]
pub struct BroadcastSink {
    tx: broadcast::Sender<RunEvent>,
}

impl BroadcastSink {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RunEvent> {
        self.tx.subscribe()
    }
}

impl EventSink for BroadcastSink {
    fn emit(&self, event: RunEvent) {
        // No subscribers is fine.
        let _ = self.tx.send(event);
    }
}

/// Structured run log: kept in the result *and* streamed as events.
pub struct RunLog {
    run_id: String,
    sink: Arc<dyn EventSink>,
    entries: Mutex<Vec<LogEntry>>,
}

impl RunLog {
    pub fn new(run_id: &str, sink: Arc<dyn EventSink>) -> Self {
        Self { run_id: run_id.to_string(), sink, entries: Mutex::new(Vec::new()) }
    }

    pub fn log(&self, level: LogLevel, scope: &str, message: impl Into<String>) {
        let entry = LogEntry { ts: Utc::now(), level, scope: scope.to_string(), message: message.into() };
        match level {
            LogLevel::Error => tracing::error!(scope, "{}", entry.message),
            LogLevel::Warn => tracing::warn!(scope, "{}", entry.message),
            LogLevel::Info => tracing::info!(scope, "{}", entry.message),
        }
        self.entries.lock().unwrap().push(entry.clone());
        self.sink.emit(RunEvent::Log { run_id: self.run_id.clone(), entry });
    }

    pub fn info(&self, scope: &str, message: impl Into<String>) {
        self.log(LogLevel::Info, scope, message);
    }

    pub fn warn(&self, scope: &str, message: impl Into<String>) {
        self.log(LogLevel::Warn, scope, message);
    }

    pub fn error(&self, scope: &str, message: impl Into<String>) {
        self.log(LogLevel::Error, scope, message);
    }

    pub fn entries(&self) -> Vec<LogEntry> {
        self.entries.lock().unwrap().clone()
    }
}
