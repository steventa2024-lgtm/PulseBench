//! Loading, validating and fingerprinting benchmark suites.
//!
//! Layout:
//! ```text
//! <suite>/
//!   suite.json
//!   suite.lock.json          (official suites: content hash for the declared version)
//!   toolchain/package-lock.json   (optional, pins the Node toolchain)
//!   tasks/<task-id>/
//!     task.json
//!     workspace/             (runnable fixture the model sees)
//!     solution/              (reference solution, used to validate the task itself)
//!     mutants/...            (broken implementations, for "write tests" tasks)
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use pulsebench_sandbox::{normalize_rel_path, policy};
use pulsebench_types::{Runtime, SuiteManifest, SuiteSummary, Task, TaskSummary, Verification, SUITE_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{io_err, CoreError, Result};

pub const LOCK_FILE: &str = "suite.lock.json";

#[derive(Debug, Clone)]
pub struct LoadedTask {
    pub task: Task,
    pub dir: PathBuf,
}

impl LoadedTask {
    pub fn fixture_dir(&self) -> PathBuf {
        self.dir.join(&self.task.workspace)
    }

    pub fn solution_dir(&self) -> PathBuf {
        self.dir.join("solution")
    }
}

#[derive(Debug, Clone)]
pub struct LoadedSuite {
    pub manifest: SuiteManifest,
    pub dir: PathBuf,
    pub tasks: Vec<LoadedTask>,
    pub content_hash: String,
    pub lock: LockStatus,
    pub node_lockfile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockStatus {
    Verified,
    Mismatch { expected: String, actual: String },
    Missing,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockFile {
    pub suite_id: String,
    pub version: String,
    pub content_hash: String,
    pub files: u32,
}

impl LoadedSuite {
    /// Official status is only claimed when the content matches the committed lock.
    pub fn is_verified_official(&self) -> bool {
        self.manifest.official && self.lock == LockStatus::Verified
    }

    pub fn runtimes(&self) -> Vec<Runtime> {
        let set: BTreeSet<u8> = self.tasks.iter().map(|t| if t.task.language.runtime() == Runtime::Python { 0 } else { 1 }).collect();
        set.into_iter().map(|i| if i == 0 { Runtime::Python } else { Runtime::Node }).collect()
    }

    pub fn task(&self, id: &str) -> Option<&LoadedTask> {
        self.tasks.iter().find(|t| t.task.id == id)
    }

    pub fn summary(&self) -> SuiteSummary {
        let mut categories: BTreeMap<String, u32> = BTreeMap::new();
        let mut languages: BTreeMap<String, u32> = BTreeMap::new();
        for t in &self.tasks {
            *categories.entry(enum_name(&t.task.category)).or_default() += 1;
            *languages.entry(enum_name(&t.task.language)).or_default() += 1;
        }
        SuiteSummary {
            id: self.manifest.id.clone(),
            name: self.manifest.name.clone(),
            version: self.manifest.version.clone(),
            description: self.manifest.description.clone(),
            official: self.is_verified_official(),
            task_count: self.tasks.len() as u32,
            categories,
            languages,
            content_hash: self.content_hash.clone(),
            lock_verified: self.manifest.official.then(|| self.lock == LockStatus::Verified),
            estimated_minutes: self.manifest.estimated_minutes.clone(),
            path: self.dir.display().to_string(),
            tasks: self
                .tasks
                .iter()
                .map(|t| TaskSummary {
                    id: t.task.id.clone(),
                    title: t.task.title.clone(),
                    category: t.task.category,
                    language: t.task.language,
                    difficulty: t.task.difficulty,
                })
                .collect(),
            requires: self.runtimes(),
        }
    }
}

fn enum_name<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).map_err(io_err(path))?;
    serde_json::from_str(&text).map_err(|source| CoreError::Json { path: path.display().to_string(), source })
}

/// Load and validate a suite directory.
pub fn load_suite(dir: &Path) -> Result<LoadedSuite> {
    let manifest: SuiteManifest = read_json(&dir.join("suite.json"))?;
    if manifest.schema_version != SUITE_SCHEMA_VERSION {
        return Err(CoreError::Suite(format!(
            "{}: unsupported suite schemaVersion {} (this PulseBench understands {SUITE_SCHEMA_VERSION})",
            dir.display(),
            manifest.schema_version
        )));
    }
    if manifest.id.is_empty() || !manifest.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(CoreError::Suite(format!("suite id '{}' must be alphanumeric, '-' or '_'", manifest.id)));
    }
    if manifest.tasks.is_empty() {
        return Err(CoreError::Suite("suite lists no tasks".into()));
    }
    let mut seen = BTreeSet::new();
    let mut tasks = Vec::new();
    for name in &manifest.tasks {
        if !seen.insert(name.clone()) {
            return Err(CoreError::Suite(format!("task '{name}' is listed twice")));
        }
        if name.contains('/') || name.contains('\\') || name.starts_with('.') {
            return Err(CoreError::Suite(format!("invalid task directory name '{name}'")));
        }
        let tdir = dir.join("tasks").join(name);
        let task: Task = read_json(&tdir.join("task.json"))?;
        let loaded = LoadedTask { task, dir: tdir };
        validate_task(&loaded, name)?;
        tasks.push(loaded);
    }

    let (content_hash, _files) = hash_dir(dir)?;
    let lock = match read_json::<LockFile>(&dir.join(LOCK_FILE)) {
        Ok(l) if l.content_hash == content_hash && l.version == manifest.version && l.suite_id == manifest.id => LockStatus::Verified,
        Ok(l) => LockStatus::Mismatch { expected: l.content_hash, actual: content_hash.clone() },
        Err(_) => LockStatus::Missing,
    };
    let node_lockfile = fs::read_to_string(dir.join("toolchain").join("package-lock.json")).ok();
    Ok(LoadedSuite { manifest, dir: dir.to_path_buf(), tasks, content_hash, lock, node_lockfile })
}

fn bad(task: &str, reason: impl Into<String>) -> CoreError {
    CoreError::Task { task: task.to_string(), reason: reason.into() }
}

fn validate_task(t: &LoadedTask, dir_name: &str) -> Result<()> {
    let task = &t.task;
    let id = task.id.as_str();
    if id != dir_name {
        return Err(bad(dir_name, format!("task.json id '{id}' must equal its directory name")));
    }
    if !(1..=3600).contains(&task.timeout_seconds) {
        return Err(bad(id, "timeoutSeconds must be between 1 and 3600"));
    }
    if !(1..=5).contains(&task.max_attempts) {
        return Err(bad(id, "maxAttempts must be between 1 and 5"));
    }
    let fixture = t.fixture_dir();
    if !fixture.is_dir() {
        return Err(bad(id, format!("workspace directory '{}' not found", task.workspace)));
    }
    if task.entry_files.is_empty() {
        return Err(bad(id, "entryFiles must not be empty"));
    }
    let editable = task.editable();
    for f in editable {
        normalize_rel_path(f).map_err(|e| bad(id, e.to_string()))?;
        if f.starts_with("node_modules") {
            return Err(bad(id, "node_modules cannot be editable"));
        }
    }
    for f in task.entry_files.iter().chain(task.context_files.iter()) {
        let rel = normalize_rel_path(f).map_err(|e| bad(id, e.to_string()))?;
        if !fixture.join(&rel).is_file() {
            return Err(bad(id, format!("file '{f}' is not present in the workspace fixture")));
        }
    }
    for cmd in task.compile_command.iter().chain(std::iter::once(&task.test_command)) {
        let parsed = policy::parse_and_validate(cmd).map_err(|e| bad(id, e.to_string()))?;
        let is_python_task = task.language.runtime() == Runtime::Python;
        if is_python_task && parsed.program == "node" {
            return Err(bad(id, "a Python task cannot use node commands"));
        }
    }
    if let Verification::Mutation { mutants, .. } = &task.verification {
        if mutants.is_empty() {
            return Err(bad(id, "mutation verification needs at least one mutant"));
        }
        for m in mutants {
            if m.overlay.is_empty() {
                return Err(bad(id, format!("mutant '{}' has an empty overlay", m.id)));
            }
            for (target, src) in &m.overlay {
                normalize_rel_path(target).map_err(|e| bad(id, e.to_string()))?;
                let rel = normalize_rel_path(src).map_err(|e| bad(id, e.to_string()))?;
                if !t.dir.join(rel).is_file() {
                    return Err(bad(id, format!("mutant '{}' overlay file '{src}' not found", m.id)));
                }
            }
        }
    }
    Ok(())
}

/// Hash every file in the suite directory (except the lock file), normalizing CRLF so Windows
/// checkouts hash identically to Linux ones.
pub fn hash_dir(dir: &Path) -> Result<(String, u32)> {
    let mut files = Vec::new();
    collect_files(dir, dir, &mut files)?;
    files.sort();
    let mut h = Sha256::new();
    for rel in &files {
        let bytes = fs::read(dir.join(rel)).map_err(io_err(&dir.join(rel)))?;
        h.update((rel.len() as u64).to_le_bytes());
        h.update(rel.as_bytes());
        let normalized = normalize_eol(&bytes);
        h.update((normalized.len() as u64).to_le_bytes());
        h.update(&normalized);
    }
    Ok((h.finalize().iter().map(|b| format!("{b:02x}")).collect(), files.len() as u32))
}

fn normalize_eol(bytes: &[u8]) -> Vec<u8> {
    if bytes.contains(&0) {
        return bytes.to_vec();
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for e in fs::read_dir(dir).map_err(io_err(dir))? {
        let e = e.map_err(io_err(dir))?;
        let name = e.file_name().to_string_lossy().into_owned();
        let ty = e.file_type().map_err(io_err(dir))?;
        if name == LOCK_FILE && dir == root
            || name == ".DS_Store"
            || name == "__pycache__"
            || name == "node_modules"
            || name.starts_with(".pb-")
        {
            continue;
        }
        if ty.is_dir() {
            collect_files(root, &e.path(), out)?;
        } else if ty.is_file() {
            let rel = e.path().strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.push(rel);
        }
    }
    Ok(())
}

/// Write `suite.lock.json` for the suite's current content.
pub fn write_lock(dir: &Path) -> Result<LockFile> {
    let suite = load_suite(dir)?;
    let (hash, files) = hash_dir(dir)?;
    let lock = LockFile { suite_id: suite.manifest.id.clone(), version: suite.manifest.version.clone(), content_hash: hash, files };
    let text = serde_json::to_string_pretty(&lock).expect("lock serializes") + "\n";
    fs::write(dir.join(LOCK_FILE), text).map_err(io_err(&dir.join(LOCK_FILE)))?;
    Ok(lock)
}

/// Discover suites in `roots` (each root contains one directory per suite).
/// Invalid suites are returned as errors alongside the valid ones so the UI can show them.
pub fn discover_suites(roots: &[PathBuf]) -> (Vec<LoadedSuite>, Vec<(PathBuf, CoreError)>) {
    let mut suites: Vec<LoadedSuite> = Vec::new();
    let mut errors = Vec::new();
    for root in roots {
        let Ok(rd) = fs::read_dir(root) else { continue };
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.join("suite.json").is_file()).collect();
        dirs.sort();
        for d in dirs {
            match load_suite(&d) {
                Ok(s) => {
                    // The first root wins on id collisions (bundled suites cannot be shadowed by accident).
                    if !suites.iter().any(|x| x.manifest.id == s.manifest.id) {
                        suites.push(s);
                    }
                }
                Err(e) => errors.push((d, e)),
            }
        }
    }
    (suites, errors)
}
