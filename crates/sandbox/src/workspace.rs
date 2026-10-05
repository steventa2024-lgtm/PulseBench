//! Disposable benchmark workspaces and safe file writes.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{Result, SandboxError};

/// Largest single file a model may write.
pub const MAX_WRITE_BYTES: usize = 1024 * 1024;

/// A fresh copy of a task fixture. Removed on [`Workspace::destroy`].
#[derive(Debug)]
pub struct Workspace {
    root: PathBuf,
    linked_node_modules: bool,
}

impl Workspace {
    /// Copy `fixture` into a new directory under `base`.
    pub fn create(base: &Path, label: &str, fixture: &Path) -> Result<Workspace> {
        if !fixture.is_dir() {
            return Err(SandboxError::Workspace(format!("fixture directory not found: {}", fixture.display())));
        }
        let safe_label: String = label.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
        let root = base.join(format!("{}-{}", safe_label, &uuid::Uuid::new_v4().simple().to_string()[..8]));
        fs::create_dir_all(&root)?;
        copy_dir(fixture, &root)?;
        fs::create_dir_all(root.join(".pb-home"))?;
        fs::create_dir_all(root.join(".pb-tmp"))?;
        Ok(Workspace { root, linked_node_modules: false })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Make a shared, read-only `node_modules` visible inside the workspace.
    pub fn link_node_modules(&mut self, target: &Path) -> Result<()> {
        let link = self.root.join("node_modules");
        if link.exists() {
            return Err(SandboxError::Workspace("fixture must not contain node_modules".into()));
        }
        link_dir(target, &link)?;
        self.linked_node_modules = true;
        Ok(())
    }

    pub fn read_text(&self, rel: &str) -> Result<Option<String>> {
        let rel = normalize_rel_path(rel)?;
        let p = self.root.join(&rel);
        if !p.exists() {
            return Ok(None);
        }
        match fs::read(&p) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(e) => Err(e.into()),
        }
    }

    /// Write a UTF-8 file at a workspace-relative path, refusing anything that could escape.
    pub fn write_text(&self, rel: &str, content: &str) -> Result<()> {
        let rel = normalize_rel_path(rel)?;
        if content.len() > MAX_WRITE_BYTES {
            return Err(SandboxError::UnsafePath { path: rel, reason: format!("file larger than {MAX_WRITE_BYTES} bytes") });
        }
        let first = rel.split('/').next().unwrap_or("");
        if first == "node_modules" || first.starts_with(".pb-") {
            return Err(SandboxError::UnsafePath { path: rel, reason: "reserved location".into() });
        }
        let target = self.root.join(&rel);
        // No existing component of the path may be a symlink/junction.
        let mut cur = self.root.clone();
        for comp in rel.split('/') {
            cur.push(comp);
            if let Ok(meta) = fs::symlink_metadata(&cur) {
                if meta.file_type().is_symlink() {
                    return Err(SandboxError::UnsafePath { path: rel, reason: "path traverses a symbolic link".into() });
                }
            }
        }
        if target.is_dir() {
            return Err(SandboxError::UnsafePath { path: rel, reason: "is a directory".into() });
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        // Defence in depth: the canonical parent must still be inside the workspace.
        let canon_root = fs::canonicalize(&self.root)?;
        let canon_parent = fs::canonicalize(target.parent().unwrap_or(&self.root))?;
        if !canon_parent.starts_with(&canon_root) {
            return Err(SandboxError::UnsafePath { path: rel, reason: "escapes the workspace".into() });
        }
        fs::write(&target, content)?;
        Ok(())
    }

    /// Replace files with versions from outside the workspace (used for mutation testing).
    /// Returns the previous contents so they can be restored.
    pub fn overlay(&self, files: &[(String, PathBuf)]) -> Result<Vec<(String, Option<String>)>> {
        let mut backup = Vec::new();
        for (rel, src) in files {
            let rel = normalize_rel_path(rel)?;
            let prev = self.read_text(&rel)?;
            let content = fs::read_to_string(src)?;
            self.write_text(&rel, &content)?;
            backup.push((rel, prev));
        }
        Ok(backup)
    }

    pub fn restore(&self, backup: Vec<(String, Option<String>)>) -> Result<()> {
        for (rel, prev) in backup {
            match prev {
                Some(content) => self.write_text(&rel, &content)?,
                None => {
                    let _ = fs::remove_file(self.root.join(&rel));
                }
            }
        }
        Ok(())
    }

    /// Remove the workspace (never follows the node_modules link).
    pub fn destroy(self) {
        if self.linked_node_modules {
            let link = self.root.join("node_modules");
            #[cfg(windows)]
            let _ = fs::remove_dir(&link);
            #[cfg(not(windows))]
            let _ = fs::remove_file(&link);
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Validate and normalize a workspace-relative path to forward slashes.
pub fn normalize_rel_path(raw: &str) -> Result<String> {
    let bad = |reason: &str| SandboxError::UnsafePath { path: raw.to_string(), reason: reason.into() };
    if raw.is_empty() {
        return Err(bad("empty path"));
    }
    if raw.contains('\0') {
        return Err(bad("NUL byte"));
    }
    let unified = raw.replace('\\', "/");
    if unified.ends_with('/') {
        return Err(bad("path names a directory"));
    }
    if unified.starts_with('/') || unified.starts_with("//") {
        return Err(bad("absolute path"));
    }
    // Drive letters (`C:`) and URI-ish prefixes.
    if unified.split('/').next().map(|c| c.contains(':')).unwrap_or(false) {
        return Err(bad("drive or scheme prefix"));
    }
    let mut parts: Vec<&str> = Vec::new();
    for comp in Path::new(&unified).components() {
        match comp {
            Component::Normal(s) => parts.push(s.to_str().ok_or_else(|| bad("non-UTF-8 path"))?),
            Component::CurDir => {}
            Component::ParentDir => return Err(bad("parent directory reference")),
            Component::RootDir | Component::Prefix(_) => return Err(bad("absolute path")),
        }
    }
    if parts.is_empty() {
        return Err(bad("empty path"));
    }
    if parts.iter().any(|p| p.is_empty() || p.ends_with('.') || p.ends_with(' ')) {
        return Err(bad("invalid path component"));
    }
    Ok(parts.join("/"))
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let name = entry.file_name();
        let to = dst.join(&name);
        if ty.is_symlink() {
            // Fixtures are plain files; refuse to materialize links.
            continue;
        }
        if ty.is_dir() {
            fs::create_dir_all(&to)?;
            copy_dir(&entry.path(), &to)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn link_dir(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(windows)]
pub(crate) fn link_dir(target: &Path, link: &Path) -> Result<()> {
    // A junction does not need elevated privileges, unlike a symlink.
    let mut cmd = std::process::Command::new("cmd");
    cmd.args(["/C", "mklink", "/J"]).arg(link).arg(target);
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
    let out = cmd.output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(SandboxError::Workspace(format!("mklink /J failed: {}", String::from_utf8_lossy(&out.stderr))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("src")).unwrap();
        fs::write(d.path().join("src/a.txt"), "original").unwrap();
        d
    }

    #[test]
    fn normalizes_and_rejects_paths() {
        assert_eq!(normalize_rel_path("./src//a.py").unwrap(), "src/a.py");
        assert_eq!(normalize_rel_path("src\\a.py").unwrap(), "src/a.py");
        for bad in ["../x", "src/../../x", "/etc/passwd", "C:\\x", "C:/x", "", ".", "a/./..", "\\\\server\\share", "a\0b", "src/", "x/ "] {
            assert!(normalize_rel_path(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn workspace_is_a_clean_copy_and_destroyable() {
        let fx = fixture();
        let base = tempfile::tempdir().unwrap();
        let a = Workspace::create(base.path(), "task a", fx.path()).unwrap();
        let b = Workspace::create(base.path(), "task a", fx.path()).unwrap();
        a.write_text("src/a.txt", "changed by model A").unwrap();
        assert_eq!(b.read_text("src/a.txt").unwrap().as_deref(), Some("original"), "workspaces must be independent");
        assert_eq!(fs::read_to_string(fx.path().join("src/a.txt")).unwrap(), "original", "fixture must stay untouched");
        let root = a.root().to_path_buf();
        a.destroy();
        assert!(!root.exists());
    }

    #[test]
    fn writes_cannot_escape_or_touch_reserved_paths() {
        let fx = fixture();
        let base = tempfile::tempdir().unwrap();
        let ws = Workspace::create(base.path(), "t", fx.path()).unwrap();
        assert!(ws.write_text("../evil.txt", "x").is_err());
        assert!(ws.write_text("node_modules/pkg/index.js", "x").is_err());
        assert!(ws.write_text(".pb-home/x", "x").is_err());
        assert!(ws.write_text("src", "x").is_err(), "cannot overwrite a directory");
        assert!(ws.write_text("new/dir/file.py", "ok").is_ok());
        assert!(!base.path().join("evil.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn writes_through_symlinks_are_refused() {
        let fx = fixture();
        let base = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let ws = Workspace::create(base.path(), "t", fx.path()).unwrap();
        std::os::unix::fs::symlink(outside.path(), ws.root().join("link")).unwrap();
        assert!(ws.write_text("link/pwned.txt", "x").is_err());
        assert!(!outside.path().join("pwned.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn fixture_symlinks_are_not_copied() {
        let fx = fixture();
        std::os::unix::fs::symlink("/etc/passwd", fx.path().join("pw")).unwrap();
        let base = tempfile::tempdir().unwrap();
        let ws = Workspace::create(base.path(), "t", fx.path()).unwrap();
        assert!(!ws.root().join("pw").exists());
    }

    #[test]
    fn overlay_and_restore_round_trip() {
        let fx = fixture();
        let base = tempfile::tempdir().unwrap();
        let ws = Workspace::create(base.path(), "t", fx.path()).unwrap();
        let mutant = base.path().join("mutant.txt");
        fs::write(&mutant, "mutant").unwrap();
        let backup = ws.overlay(&[("src/a.txt".into(), mutant)]).unwrap();
        assert_eq!(ws.read_text("src/a.txt").unwrap().as_deref(), Some("mutant"));
        ws.restore(backup).unwrap();
        assert_eq!(ws.read_text("src/a.txt").unwrap().as_deref(), Some("original"));
    }
}
