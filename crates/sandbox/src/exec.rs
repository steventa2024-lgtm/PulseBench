//! Process execution with timeouts, output caps and process-tree termination.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio_util::sync::CancellationToken;

/// Hard cap on bytes buffered per stream; anything beyond is dropped.
const READ_CAP: usize = 2 * 1024 * 1024;
/// Text kept per stream in results (head + tail).
const KEEP_HEAD: usize = 48 * 1024;
const KEEP_TAIL: usize = 48 * 1024;

#[derive(Debug, Clone)]
pub struct ExecRequest {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// The complete environment. Nothing is inherited.
    pub env: Vec<(String, String)>,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub duration: Duration,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
}

impl ExecOutput {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out && !self.cancelled
    }
}

pub async fn run(req: ExecRequest, cancel: &CancellationToken) -> std::io::Result<ExecOutput> {
    let started = Instant::now();
    let mut cmd = Command::new(&req.program);
    cmd.args(&req.args)
        .current_dir(&req.cwd)
        .env_clear()
        .envs(req.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_new_group(&mut cmd);

    let mut child = cmd.spawn()?;
    let pid = child.id();
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let out_task = tokio::spawn(async move { read_capped(&mut stdout).await });
    let err_task = tokio::spawn(async move { read_capped(&mut stderr).await });

    let mut timed_out = false;
    let mut cancelled = false;
    let status = tokio::select! {
        s = child.wait() => Some(s?),
        _ = tokio::time::sleep(req.timeout) => { timed_out = true; None }
        _ = cancel.cancelled() => { cancelled = true; None }
    };
    let exit_code = match status {
        Some(s) => {
            // The direct child exited; make sure no background grandchildren linger.
            kill_tree(pid, &mut child).await;
            s.code()
        }
        None => {
            kill_tree(pid, &mut child).await;
            let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
            None
        }
    };

    let (out, out_trunc) = join_reader(out_task).await;
    let (err, err_trunc) = join_reader(err_task).await;
    let (stdout, t1) = trim_middle(out);
    let (stderr, t2) = trim_middle(err);
    Ok(ExecOutput {
        exit_code,
        timed_out,
        cancelled,
        duration: started.elapsed(),
        stdout,
        stderr,
        truncated: out_trunc || err_trunc || t1 || t2,
    })
}

async fn join_reader(h: tokio::task::JoinHandle<(Vec<u8>, bool)>) -> (Vec<u8>, bool) {
    // After the tree is killed the pipes close; guard against a stray holder anyway.
    match tokio::time::timeout(Duration::from_secs(3), h).await {
        Ok(Ok(v)) => v,
        _ => (Vec::new(), true),
    }
}

async fn read_capped<R: tokio::io::AsyncRead + Unpin>(r: &mut R) -> (Vec<u8>, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    let mut truncated = false;
    loop {
        match r.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if buf.len() < READ_CAP {
                    let take = n.min(READ_CAP - buf.len());
                    buf.extend_from_slice(&chunk[..take]);
                    if take < n {
                        truncated = true;
                    }
                } else {
                    truncated = true;
                }
            }
        }
    }
    (buf, truncated)
}

fn trim_middle(bytes: Vec<u8>) -> (String, bool) {
    if bytes.len() <= KEEP_HEAD + KEEP_TAIL {
        return (String::from_utf8_lossy(&bytes).into_owned(), false);
    }
    let head = String::from_utf8_lossy(&bytes[..KEEP_HEAD]);
    let tail = String::from_utf8_lossy(&bytes[bytes.len() - KEEP_TAIL..]);
    let dropped = bytes.len() - KEEP_HEAD - KEEP_TAIL;
    (format!("{head}\n…[{dropped} bytes omitted]…\n{tail}"), true)
}

#[cfg(unix)]
fn configure_new_group(cmd: &mut Command) {
    // New process group so the whole tree can be signalled at once.
    cmd.process_group(0);
}

#[cfg(windows)]
fn configure_new_group(cmd: &mut Command) {
    // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
    cmd.creation_flags(0x0800_0000 | 0x0000_0200);
}

#[cfg(not(any(unix, windows)))]
fn configure_new_group(_cmd: &mut Command) {}

/// Kill the child and every descendant.
async fn kill_tree(pid: Option<u32>, child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        // SAFETY: plain signal syscall on the process group we created (pgid == pid).
        unsafe {
            libc::killpg(pid as i32, libc::SIGKILL);
        }
    }
    #[cfg(windows)]
    if let Some(pid) = pid {
        let mut k = std::process::Command::new("taskkill");
        k.args(["/PID", &pid.to_string(), "/T", "/F"]).stdout(Stdio::null()).stderr(Stdio::null());
        std::os::windows::process::CommandExt::creation_flags(&mut k, 0x0800_0000);
        let _ = tokio::task::spawn_blocking(move || k.status()).await;
    }
    let _ = pid;
    let _ = child.start_kill();
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str, timeout: Duration) -> ExecRequest {
        ExecRequest {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), script.into()],
            cwd: std::env::temp_dir(),
            env: vec![("PATH".into(), "/usr/bin:/bin".into())],
            timeout,
        }
    }

    #[tokio::test]
    async fn captures_output_and_exit_code() {
        let out = run(sh("echo hi; echo err >&2; exit 3", Duration::from_secs(10)), &CancellationToken::new()).await.unwrap();
        assert_eq!(out.exit_code, Some(3));
        assert_eq!(out.stdout.trim(), "hi");
        assert_eq!(out.stderr.trim(), "err");
        assert!(!out.timed_out);
    }

    #[tokio::test]
    async fn does_not_inherit_environment() {
        std::env::set_var("PULSEBENCH_TEST_SECRET", "hunter2");
        let out = run(sh("echo \"[$PULSEBENCH_TEST_SECRET]\"", Duration::from_secs(10)), &CancellationToken::new()).await.unwrap();
        assert_eq!(out.stdout.trim(), "[]");
    }

    #[tokio::test]
    async fn timeout_kills_process_tree() {
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join("child.pid");
        let script = format!("sleep 60 & echo $! > {}; wait", pidfile.display());
        let started = Instant::now();
        let out = run(sh(&script, Duration::from_millis(600)), &CancellationToken::new()).await.unwrap();
        assert!(out.timed_out);
        assert!(started.elapsed() < Duration::from_secs(10));
        let pid: i32 = std::fs::read_to_string(&pidfile).unwrap().trim().parse().unwrap();
        // Give the kernel a moment to reap, then verify the grandchild is gone.
        tokio::time::sleep(Duration::from_millis(200)).await;
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        // A zombie still answers kill(0); check /proc state when available.
        let zombie = std::fs::read_to_string(format!("/proc/{pid}/stat")).map(|s| s.contains(") Z")).unwrap_or(false);
        assert!(!alive || zombie, "grandchild {pid} survived the timeout");
    }

    #[tokio::test]
    async fn cancellation_stops_a_running_command() {
        let cancel = CancellationToken::new();
        let c = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            c.cancel();
        });
        let started = Instant::now();
        let out = run(sh("sleep 60", Duration::from_secs(120)), &cancel).await.unwrap();
        assert!(out.cancelled);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn huge_output_is_capped() {
        let out = run(sh("yes | head -c 5000000", Duration::from_secs(20)), &CancellationToken::new()).await.unwrap();
        assert!(out.truncated);
        assert!(out.stdout.len() < 120 * 1024);
        assert!(out.stdout.contains("bytes omitted"));
    }
}
