//! Shared, read-only Node toolchain cache (TypeScript, React, jsdom, ...).
//!
//! Installed once per unique package set by a *trusted* `npm ci`/`npm install` that is not part of
//! any model's execution, then linked read-only into every workspace.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use pulsebench_types::NodeToolchain;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use crate::error::{Result, SandboxError};
use crate::exec::{self, ExecRequest};
use crate::runtime::resolve_program;

const OK_MARKER: &str = ".pulsebench-toolchain-ok";

/// Environment variables forwarded to the trusted install step only (corporate proxies/CAs).
const INSTALL_PASSTHROUGH: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "NODE_EXTRA_CA_CERTS",
    "SSL_CERT_FILE",
    "SYSTEMROOT",
    "SystemRoot",
    "COMSPEC",
    "PATHEXT",
];

/// An install in progress holds `<cache>/node-<key>.lock` (a directory: creation is atomic across
/// threads and processes). A lock older than this is assumed to belong to a crashed process.
const STALE_LOCK: Duration = Duration::from_secs(30 * 60);

struct LockGuard(PathBuf);

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

/// Wait until we own the install lock, or until another installer finished the toolchain
/// (then `Ok(None)`). Concurrent `prepare` calls (parallel runs, UI + CLI) must never install
/// into the same directory at the same time.
async fn acquire_install_lock(lock: &Path, dir: &Path, cancel: &CancellationToken) -> Result<Option<LockGuard>> {
    loop {
        if node_toolchain_ready(dir) {
            return Ok(None);
        }
        match fs::create_dir(lock) {
            Ok(()) => return Ok(Some(LockGuard(lock.to_path_buf()))),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let stale = fs::metadata(lock)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .map(|age| age > STALE_LOCK)
                    .unwrap_or(false);
                if stale {
                    let _ = fs::remove_dir(lock);
                    continue;
                }
                tokio::select! {
                    _ = cancel.cancelled() => return Err(SandboxError::Toolchain("cancelled".into())),
                    _ = tokio::time::sleep(Duration::from_millis(250)) => {}
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}

pub fn node_toolchain_dir(cache_root: &Path, spec: &NodeToolchain, lockfile: Option<&str>) -> PathBuf {
    let mut h = Sha256::new();
    h.update(b"pulsebench-node-toolchain-v1\n");
    for (name, version) in &spec.packages {
        h.update(format!("{name}@{version}\n").as_bytes());
    }
    if let Some(lock) = lockfile {
        h.update(lock.as_bytes());
    }
    let key: String = h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect();
    cache_root.join(format!("node-{key}"))
}

pub fn node_toolchain_ready(dir: &Path) -> bool {
    dir.join("node_modules").is_dir() && dir.join(OK_MARKER).exists()
}

/// Install the toolchain if it is not cached yet. Returns the toolchain directory
/// (its `node_modules` is what gets linked into workspaces).
pub async fn prepare_node_toolchain(
    cache_root: &Path,
    spec: &NodeToolchain,
    lockfile: Option<&str>,
    cancel: &CancellationToken,
    log: &(dyn Fn(String) + Send + Sync),
) -> Result<PathBuf> {
    let dir = node_toolchain_dir(cache_root, spec, lockfile);
    if node_toolchain_ready(&dir) {
        return Ok(dir);
    }
    fs::create_dir_all(cache_root)?;
    let lock_path = cache_root.join(format!("{}.lock", dir.file_name().and_then(|n| n.to_str()).unwrap_or("node")));
    let _lock = match acquire_install_lock(&lock_path, &dir, cancel).await? {
        Some(guard) => guard,
        None => return Ok(dir), // another installer finished while we waited
    };
    let npm =
        resolve_program("npm").await.ok_or_else(|| SandboxError::MissingRuntime("npm was not found on PATH (install Node.js)".into()))?;

    log(format!("Installing Node toolchain ({} packages) — one-time, needs network access", spec.packages.len()));
    if dir.exists() {
        make_writable(&dir);
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    let pkg = serde_json::json!({ "name": "pulsebench-toolchain", "private": true, "version": "1.0.0", "dependencies": spec.packages });
    fs::write(dir.join("package.json"), serde_json::to_string_pretty(&pkg).unwrap())?;
    if let Some(lock) = lockfile {
        fs::write(dir.join("package-lock.json"), lock)?;
    }
    let home = dir.join(".home");
    fs::create_dir_all(&home)?;

    let mut env: Vec<(String, String)> = vec![
        ("PATH".into(), std::env::var("PATH").unwrap_or_default()),
        ("HOME".into(), home.display().to_string()),
        ("USERPROFILE".into(), home.display().to_string()),
        ("APPDATA".into(), home.join("AppData").display().to_string()),
        ("npm_config_update_notifier".into(), "false".into()),
    ];
    for key in INSTALL_PASSTHROUGH {
        if let Ok(v) = std::env::var(key) {
            env.push(((*key).to_string(), v));
        }
    }
    let sub = if lockfile.is_some() { "ci" } else { "install" };
    let mut args = [npm.prefix_args.clone(), vec![sub.to_string()]].concat();
    args.extend(["--no-audit", "--no-fund", "--ignore-scripts", "--loglevel=error"].map(String::from));
    if lockfile.is_none() {
        args.push("--no-package-lock".into());
    }
    let out = exec::run(ExecRequest { program: npm.path, args, cwd: dir.clone(), env, timeout: Duration::from_secs(900) }, cancel).await?;
    if !out.success() {
        let tail: String = out.stderr.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        let _ = fs::remove_dir_all(&dir);
        return Err(SandboxError::Toolchain(if out.timed_out {
            "npm install timed out".into()
        } else if out.cancelled {
            "cancelled".into()
        } else {
            format!("npm exited with {:?}: {tail}", out.exit_code)
        }));
    }
    let _ = fs::remove_dir_all(dir.join(".home"));
    make_readonly(&dir.join("node_modules"));
    fs::write(dir.join(OK_MARKER), "ok")?;
    log("Node toolchain ready".into());
    Ok(dir)
}

/// Remove write permission so benchmark code cannot modify the shared toolchain.
fn make_readonly(root: &Path) {
    walk(root, &|p, is_dir| set_mode(p, is_dir, true));
}

fn make_writable(root: &Path) {
    walk(root, &|p, is_dir| set_mode(p, is_dir, false));
}

fn walk(root: &Path, f: &dyn Fn(&Path, bool)) {
    let Ok(rd) = fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let Ok(ty) = e.file_type() else { continue };
        if ty.is_symlink() {
            continue;
        }
        let p = e.path();
        if ty.is_dir() {
            walk(&p, f);
            f(&p, true);
        } else {
            f(&p, false);
        }
    }
    f(root, true);
}

#[cfg(unix)]
fn set_mode(p: &Path, is_dir: bool, readonly: bool) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(p) {
        let mut mode = meta.permissions().mode();
        if readonly {
            mode &= !0o222;
        } else {
            mode |= if is_dir { 0o700 } else { 0o600 };
        }
        let _ = fs::set_permissions(p, fs::Permissions::from_mode(mode));
    }
}

#[cfg(not(unix))]
fn set_mode(p: &Path, is_dir: bool, readonly: bool) {
    if is_dir {
        return;
    }
    if let Ok(meta) = fs::metadata(p) {
        let mut perm = meta.permissions();
        perm.set_readonly(readonly);
        let _ = fs::set_permissions(p, perm);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn install_lock_serializes_installers_and_hands_over_a_finished_toolchain() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("node-abc");
        let lock = tmp.path().join("node-abc.lock");
        let cancel = CancellationToken::new();

        let first = acquire_install_lock(&lock, &dir, &cancel).await.unwrap().expect("first caller gets the lock");
        // A second caller must wait while the first holds the lock...
        let (l2, d2, c2) = (lock.clone(), dir.clone(), cancel.clone());
        let second = tokio::spawn(async move { acquire_install_lock(&l2, &d2, &c2).await.map(|g| g.is_some()) });
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(!second.is_finished(), "second installer must wait for the lock");
        // ...and when the first finishes the toolchain it must not install again.
        fs::create_dir_all(dir.join("node_modules")).unwrap();
        fs::write(dir.join(OK_MARKER), "ok").unwrap();
        drop(first);
        assert!(!second.await.unwrap().unwrap(), "waiter sees the finished toolchain instead of reinstalling");
        assert!(!lock.exists());
    }

    #[tokio::test]
    async fn waiting_for_the_lock_is_cancellable_and_stale_locks_are_ignored_by_age_only() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("node-x");
        let lock = tmp.path().join("node-x.lock");
        fs::create_dir(&lock).unwrap(); // fresh lock held by "someone else"
        let cancel = CancellationToken::new();
        let c2 = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            c2.cancel();
        });
        let err = acquire_install_lock(&lock, &dir, &cancel).await.err().expect("must be cancelled");
        assert!(err.to_string().contains("cancelled"));
        assert!(lock.exists(), "a lock we do not own is never removed while fresh");
    }

    #[test]
    fn toolchain_dir_is_keyed_by_content() {
        let root = Path::new("/cache");
        let mut a = NodeToolchain::default();
        a.packages.insert("typescript".into(), "5.6.3".into());
        let mut b = a.clone();
        b.packages.insert("react".into(), "19.0.0".into());
        let da = node_toolchain_dir(root, &a, None);
        assert_eq!(da, node_toolchain_dir(root, &a, None));
        assert_ne!(da, node_toolchain_dir(root, &b, None));
        assert_ne!(da, node_toolchain_dir(root, &a, Some("{lock}")));
    }
}
