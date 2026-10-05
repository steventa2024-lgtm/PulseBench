use std::path::{Path, PathBuf};

/// Where PulseBench keeps its data and finds benchmark suites.
#[derive(Clone, Debug)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    /// Suites shipped with the application (read-only).
    pub bundled_suites: Vec<PathBuf>,
}

impl AppPaths {
    pub fn database(&self) -> PathBuf {
        self.data_dir.join("pulsebench.db")
    }

    pub fn work_dir(&self) -> PathBuf {
        self.data_dir.join("work")
    }

    pub fn toolchain_dir(&self) -> PathBuf {
        self.data_dir.join("toolchains")
    }

    /// Custom suites imported by the user.
    pub fn user_suites(&self) -> PathBuf {
        self.data_dir.join("benchmarks")
    }

    /// Default locations: `PULSEBENCH_DATA` / OS data dir, and bundled suites found next to the
    /// executable, in the current directory tree (development) or via `PULSEBENCH_SUITES`.
    pub fn detect(data_dir: Option<PathBuf>, extra_bundled: Option<PathBuf>) -> AppPaths {
        let data_dir = data_dir
            .or_else(|| std::env::var_os("PULSEBENCH_DATA").map(PathBuf::from))
            .unwrap_or_else(|| dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("ZeroPulse").join("PulseBench"));
        let mut bundled = Vec::new();
        if let Some(p) = extra_bundled {
            bundled.push(p);
        }
        if let Some(p) = std::env::var_os("PULSEBENCH_SUITES") {
            bundled.push(PathBuf::from(p));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for rel in ["benchmarks", "resources/benchmarks", "../Resources/benchmarks", "../lib/pulsebench/benchmarks"] {
                    bundled.push(dir.join(rel));
                }
                // Development: walk up from the binary (target/debug/...) to the repository root.
                let mut cur: Option<&Path> = Some(dir);
                for _ in 0..6 {
                    if let Some(c) = cur {
                        bundled.push(c.join("benchmarks"));
                        cur = c.parent();
                    }
                }
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            bundled.push(cwd.join("benchmarks"));
        }
        let mut seen = Vec::new();
        bundled.retain(|p| {
            let ok = p.is_dir() && !seen.contains(p);
            if ok {
                seen.push(p.clone());
            }
            ok
        });
        AppPaths { data_dir, bundled_suites: bundled }
    }
}
