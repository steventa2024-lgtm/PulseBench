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
