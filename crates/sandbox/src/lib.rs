//! Isolated execution of benchmark commands.
//!
//! Two modes are supported:
//! * **Local isolated workspace** — a throw-away directory, an empty environment (with `HOME` and
//!   temp dirs redirected into the workspace), a command allowlist, timeouts and process-tree
//!   kills. This is *not* an OS-level security boundary: code run this way can still read files the
//!   current user can read.
//! * **Docker** — `docker run --network none` with only the workspace mounted.

pub mod error;
pub mod exec;
pub mod policy;
pub mod runtime;
pub mod toolchain;
pub mod workspace;

use std::path::PathBuf;
use std::time::Duration;

use pulsebench_types::{CommandOutcome, DockerImages, ExecutionMode, Runtime};
use tokio_util::sync::CancellationToken;

pub use error::{Result, SandboxError};
pub use workspace::{normalize_rel_path, Workspace};

#[derive(Debug, Clone)]
pub struct Sandbox {
    mode: ExecutionMode,
    images: DockerImages,
    /// Parent directory for disposable workspaces.
    work_root: PathBuf,
    /// Directory containing the prepared `node_modules` (if the suite needs one).
    node_toolchain: Option<PathBuf>,
}

impl Sandbox {
    pub fn new(mode: ExecutionMode, images: DockerImages, work_root: PathBuf, node_toolchain: Option<PathBuf>) -> Self {
        Self { mode, images, work_root, node_toolchain }
    }

    pub fn mode(&self) -> ExecutionMode {
        self.mode
    }

    /// Honest, user-facing description of the isolation actually in effect.
    pub fn describe(&self) -> String {
        match self.mode {
            ExecutionMode::Local => "Local isolated workspace: disposable directory, sanitized environment, command allowlist and \
                 timeouts. Not an OS-level security boundary."
                .to_string(),
            ExecutionMode::Docker => {
                format!("Docker: {} / {}, network disabled, only the workspace is mounted.", self.images.python, self.images.node)
            }
        }
    }

    pub fn create_workspace(&self, label: &str, fixture: &std::path::Path) -> Result<Workspace> {
        std::fs::create_dir_all(&self.work_root)?;
        let mut ws = Workspace::create(&self.work_root, label, fixture)?;
        // In Docker mode the toolchain is bind-mounted instead of linked.
        if self.mode == ExecutionMode::Local {
            if let Some(tc) = &self.node_toolchain {
                ws.link_node_modules(&tc.join("node_modules"))?;
            }
        }
        Ok(ws)
    }

    /// Run a *trusted* command line (from a benchmark definition) inside the workspace.
    ///
    /// Failures to start are reported in [`CommandOutcome::spawn_error`], never swallowed.
    pub async fn run_command(&self, ws: &Workspace, command: &str, timeout: Duration, cancel: &CancellationToken) -> CommandOutcome {
        let started = std::time::Instant::now();
        let fail = |msg: String| CommandOutcome {
            command: command.to_string(),
            exit_code: None,
            timed_out: false,
            cancelled: false,
            duration_ms: started.elapsed().as_millis() as u32,
            stdout: String::new(),
            stderr: String::new(),
            truncated: false,
            spawn_error: Some(msg),
        };
        let parsed = match policy::parse_and_validate(command) {
            Ok(p) => p,
            Err(e) => return fail(e.to_string()),
        };
        let req = match self.mode {
            ExecutionMode::Local => match self.local_request(ws, &parsed, timeout).await {
                Ok(r) => r,
                Err(e) => return fail(e.to_string()),
            },
            ExecutionMode::Docker => match self.docker_request(ws, &parsed, timeout).await {
                Ok(r) => r,
                Err(e) => return fail(e.to_string()),
            },
        };
        let container = match (&self.mode, &req.1) {
            (ExecutionMode::Docker, name) => name.clone(),
            _ => None,
        };
        match exec::run(req.0, cancel).await {
            Ok(out) => {
                if let Some(name) = container {
                    if out.timed_out || out.cancelled {
                        kill_container(&name).await;
                    }
                }
                CommandOutcome {
                    command: command.to_string(),
                    exit_code: out.exit_code,
                    timed_out: out.timed_out,
                    cancelled: out.cancelled,
                    duration_ms: out.duration.as_millis() as u32,
                    stdout: out.stdout,
                    stderr: out.stderr,
                    truncated: out.truncated,
                    spawn_error: None,
                }
            }
            Err(e) => fail(format!("could not start `{}`: {e}", parsed.program)),
        }
    }

    async fn local_request(
        &self,
        ws: &Workspace,
        parsed: &policy::ParsedCommand,
        timeout: Duration,
    ) -> Result<(exec::ExecRequest, Option<String>)> {
        let resolved = runtime::resolve_program(&parsed.program)
            .await
            .ok_or_else(|| SandboxError::MissingRuntime(format!("`{}` was not found on this machine", parsed.program)))?;
        let node_bin = self.node_toolchain.as_ref().map(|t| t.join("node_modules").join(".bin"));
        let env = policy::sanitized_env(ws.root(), &std::env::var("PATH").unwrap_or_default(), node_bin.as_deref());
        Ok((
            exec::ExecRequest {
                program: resolved.path,
                args: [resolved.prefix_args, parsed.args.clone()].concat(),
                cwd: ws.root().to_path_buf(),
                env,
                timeout,
            },
            None,
        ))
    }

    async fn docker_request(
        &self,
        ws: &Workspace,
        parsed: &policy::ParsedCommand,
        timeout: Duration,
    ) -> Result<(exec::ExecRequest, Option<String>)> {
        let docker = which::which("docker").map_err(|_| SandboxError::MissingRuntime("docker was not found on PATH".into()))?;
        let runtime = match parsed.program.as_str() {
            "python" | "python3" | "py" => Runtime::Python,
            _ => Runtime::Node,
        };
        let image = match runtime {
            Runtime::Python => &self.images.python,
            Runtime::Node => &self.images.node,
        };
        let program = match parsed.program.as_str() {
            "python3" | "py" => "python",
            p => p,
        };
        let name = format!("pulsebench-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let mut args: Vec<String> = vec![
            "run".into(),
            "--rm".into(),
            "--name".into(),
            name.clone(),
            "--network".into(),
            "none".into(),
            "--memory".into(),
            "4g".into(),
            "--cpus".into(),
            "2".into(),
            "--pids-limit".into(),
            "512".into(),
            "--cap-drop".into(),
            "ALL".into(),
            "--security-opt".into(),
            "no-new-privileges".into(),
        ];
        #[cfg(unix)]
        {
            // Files written in the bind mount stay owned by the invoking user.
            // SAFETY: getuid/getgid have no preconditions.
            let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
            args.extend(["--user".into(), format!("{uid}:{gid}")]);
        }
        args.extend(["-v".into(), format!("{}:/work", ws.root().display()), "-w".into(), "/work".into()]);
        if let Some(tc) = &self.node_toolchain {
            args.extend(["-v".into(), format!("{}:/work/node_modules:ro", tc.join("node_modules").display())]);
        }
        for (k, v) in [
            ("HOME", "/work/.pb-home"),
            ("TMPDIR", "/work/.pb-tmp"),
            ("CI", "1"),
            ("NO_COLOR", "1"),
            ("NODE_ENV", "test"),
            ("PYTHONDONTWRITEBYTECODE", "1"),
            ("PYTHONHASHSEED", "0"),
            ("PYTHONUTF8", "1"),
            ("npm_config_cache", "/work/.pb-home/.npm"),
        ] {
            args.extend(["-e".into(), format!("{k}={v}")]);
        }
        args.push(image.clone());
        args.push(program.to_string());
        args.extend(parsed.args.iter().cloned());

        // The docker *client* needs a few variables to find its daemon/config; the container gets none of them.
        let mut env = Vec::new();
        for key in [
            "PATH",
            "HOME",
            "USERPROFILE",
            "APPDATA",
            "LOCALAPPDATA",
            "SystemRoot",
            "SYSTEMDRIVE",
            "WINDIR",
            "COMSPEC",
            "PATHEXT",
            "TEMP",
            "TMP",
            "DOCKER_HOST",
            "DOCKER_CONTEXT",
            "DOCKER_CONFIG",
            "DOCKER_CERT_PATH",
            "DOCKER_TLS_VERIFY",
            "XDG_RUNTIME_DIR",
        ] {
            if let Ok(v) = std::env::var(key) {
                env.push((key.to_string(), v));
            }
        }
        Ok((exec::ExecRequest { program: docker, args, cwd: ws.root().to_path_buf(), env, timeout }, Some(name)))
    }
}

async fn kill_container(name: &str) {
    if let Ok(docker) = which::which("docker") {
        let _ = runtime::capture(&docker, &["kill".into(), name.into()], Duration::from_secs(10)).await;
    }
}
