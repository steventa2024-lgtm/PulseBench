//! Locating the interpreters benchmark commands need, and Docker.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use pulsebench_types::{DockerStatus, Runtime, RuntimeStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProgram {
    pub path: PathBuf,
    /// Arguments that must precede the caller's (for example `-3` for the Windows `py` launcher).
    pub prefix_args: Vec<String>,
}

fn cache() -> &'static Mutex<HashMap<String, ResolvedProgram>> {
    static C: OnceLock<Mutex<HashMap<String, ResolvedProgram>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Resolve a logical program name (`python`, `node`, `npm`) to an executable on this machine.
pub async fn resolve_program(logical: &str) -> Option<ResolvedProgram> {
    if let Some(hit) = cache().lock().unwrap().get(logical).cloned() {
        return Some(hit);
    }
    let candidates: Vec<(&str, Vec<&str>)> = match logical {
        "python" | "python3" | "py" => vec![("python3", vec![]), ("python", vec![]), ("py", vec!["-3"])],
        other => vec![(other, vec![])],
    };
    for (name, prefix) in candidates {
        let Ok(path) = which::which(name) else { continue };
        let prefix_args: Vec<String> = prefix.iter().map(|s| s.to_string()).collect();
        let resolved = ResolvedProgram { path, prefix_args };
        let is_python = matches!(logical, "python" | "python3" | "py");
        let ok = if is_python { python_version(&resolved).await.is_some() } else { true };
        if ok {
            cache().lock().unwrap().insert(logical.to_string(), resolved.clone());
            return Some(resolved);
        }
    }
    None
}

async fn python_version(p: &ResolvedProgram) -> Option<String> {
    let out = capture(&p.path, &[p.prefix_args.clone(), vec!["--version".into()]].concat(), Duration::from_secs(5)).await?;
    // "Python 3.11.4"; reject Python 2 and the Windows Store stub (which prints nothing useful).
    let v = out.trim().strip_prefix("Python ")?.to_string();
    let major: u32 = v.split('.').next()?.parse().ok()?;
    (major >= 3).then_some(v)
}

/// Run a program and return trimmed stdout (falling back to stderr), or `None` on failure/timeout.
pub async fn capture(program: &std::path::Path, args: &[String], timeout: Duration) -> Option<String> {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let out = tokio::time::timeout(timeout, cmd.output()).await.ok()?.ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(if out.stdout.is_empty() { &out.stderr } else { &out.stdout }).trim().to_string();
    Some(text)
}

pub async fn detect_runtimes() -> Vec<RuntimeStatus> {
    let mut out = Vec::new();
    for (name, logical) in [("Python 3", "python"), ("Node.js", "node"), ("npm", "npm")] {
        match resolve_program(logical).await {
            Some(p) => {
                let version = capture(&p.path, &[p.prefix_args.clone(), vec!["--version".into()]].concat(), Duration::from_secs(8)).await;
                out.push(RuntimeStatus { name: name.into(), available: true, version, path: Some(p.path.display().to_string()) });
            }
            None => out.push(RuntimeStatus { name: name.into(), available: false, version: None, path: None }),
        }
    }
    out
}

pub async fn detect_docker() -> DockerStatus {
    let Ok(path) = which::which("docker") else {
        return DockerStatus { installed: false, running: false, version: None };
    };
    let version = capture(&path, &["version".into(), "--format".into(), "{{.Server.Version}}".into()], Duration::from_secs(6)).await;
    DockerStatus { installed: true, running: version.is_some(), version }
}

/// Runtimes a set of tasks requires, as logical program names.
pub fn runtime_program(rt: Runtime) -> &'static str {
    match rt {
        Runtime::Python => "python",
        Runtime::Node => "node",
    }
}
