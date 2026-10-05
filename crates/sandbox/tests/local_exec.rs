//! Integration tests for the local sandbox: real workspaces, real interpreters.

use std::fs;
use std::time::Duration;

use pulsebench_sandbox::Sandbox;
use pulsebench_types::{DockerImages, ExecutionMode};
use tokio_util::sync::CancellationToken;

fn sandbox(root: &std::path::Path) -> Sandbox {
    Sandbox::new(ExecutionMode::Local, DockerImages::default(), root.join("work"), None)
}

fn py_fixture(dir: &std::path::Path, impl_src: &str) {
    fs::write(dir.join("calc.py"), impl_src).unwrap();
    fs::write(
        dir.join("test_calc.py"),
        "import unittest\nfrom calc import add\nclass T(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(1, 2), 3)\n    def test_neg(self):\n        self.assertEqual(add(-1, 1), 0)\nif __name__ == '__main__':\n    unittest.main()\n",
    )
    .unwrap();
}

#[tokio::test]
async fn passing_and_failing_fixtures_report_correct_exit_codes() {
    let tmp = tempfile::tempdir().unwrap();
    let pass = tmp.path().join("pass");
    let fail = tmp.path().join("fail");
    fs::create_dir_all(&pass).unwrap();
    fs::create_dir_all(&fail).unwrap();
    py_fixture(&pass, "def add(a, b):\n    return a + b\n");
    py_fixture(&fail, "def add(a, b):\n    return a - b\n");
    let sb = sandbox(tmp.path());
    let cancel = CancellationToken::new();

    let ws = sb.create_workspace("pass", &pass).unwrap();
    let ok = sb.run_command(&ws, "python -m unittest discover -v", Duration::from_secs(60), &cancel).await;
    assert!(ok.success(), "{ok:?}");
    assert!(ok.stderr.contains("Ran 2 tests"), "{}", ok.stderr);
    ws.destroy();

    let ws = sb.create_workspace("fail", &fail).unwrap();
    let bad = sb.run_command(&ws, "python -m unittest discover -v", Duration::from_secs(60), &cancel).await;
    assert!(!bad.success());
    assert_eq!(bad.exit_code, Some(1));
    assert!(bad.stderr.contains("FAILED"), "{}", bad.stderr);
    ws.destroy();
}

#[tokio::test]
async fn timeout_is_enforced_and_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let fx = tmp.path().join("fx");
    fs::create_dir_all(&fx).unwrap();
    fs::write(fx.join("spin.py"), "import time\nwhile True:\n    time.sleep(0.1)\n").unwrap();
    let sb = sandbox(tmp.path());
    let ws = sb.create_workspace("spin", &fx).unwrap();
    let started = std::time::Instant::now();
    let out = sb.run_command(&ws, "python spin.py", Duration::from_millis(800), &CancellationToken::new()).await;
    assert!(out.timed_out, "{out:?}");
    assert!(!out.success());
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn disallowed_commands_never_run() {
    let tmp = tempfile::tempdir().unwrap();
    let fx = tmp.path().join("fx");
    fs::create_dir_all(&fx).unwrap();
    let sb = sandbox(tmp.path());
    let ws = sb.create_workspace("x", &fx).unwrap();
    for cmd in ["rm -rf .", "bash -c 'echo pwned > pwned.txt'", "python -c 'open(\"pwned.txt\",\"w\")'"] {
        let out = sb.run_command(&ws, cmd, Duration::from_secs(5), &CancellationToken::new()).await;
        assert!(out.spawn_error.is_some(), "`{cmd}` must be rejected: {out:?}");
    }
    assert!(!ws.root().join("pwned.txt").exists());
}

#[tokio::test]
async fn benchmark_processes_do_not_see_host_secrets_or_home() {
    std::env::set_var("PULSEBENCH_SECRET_TOKEN", "super-secret");
    let tmp = tempfile::tempdir().unwrap();
    let fx = tmp.path().join("fx");
    fs::create_dir_all(&fx).unwrap();
    fs::write(
        fx.join("env.py"),
        "import os\nprint('SECRET=' + str(os.environ.get('PULSEBENCH_SECRET_TOKEN')))\nprint('HOME=' + os.environ.get('HOME', ''))\n",
    )
    .unwrap();
    let sb = sandbox(tmp.path());
    let ws = sb.create_workspace("env", &fx).unwrap();
    let out = sb.run_command(&ws, "python env.py", Duration::from_secs(30), &CancellationToken::new()).await;
    assert!(out.success(), "{out:?}");
    assert!(out.stdout.contains("SECRET=None"), "{}", out.stdout);
    assert!(out.stdout.contains(&ws.root().display().to_string()), "HOME must be inside the workspace: {}", out.stdout);
}
