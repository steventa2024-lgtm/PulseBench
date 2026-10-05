//! Self-checks for benchmark tasks: the untouched fixture must fail verification and the
//! reference solution must pass it.

use std::fs;
use std::path::Path;

use pulsebench_sandbox::Sandbox;
use pulsebench_types::{DockerImages, ExecutionMode, Verification};
use tokio_util::sync::CancellationToken;

use crate::checks::run_checks;
use crate::suite::{LoadedSuite, LoadedTask};

#[derive(Debug, Clone)]
pub struct TaskValidation {
    pub task_id: String,
    pub baseline_fails: bool,
    pub solution_passes: bool,
    pub problems: Vec<String>,
}

impl TaskValidation {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

fn copy_over(src: &Path, dst: &Path) -> std::io::Result<()> {
    for e in fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            fs::create_dir_all(&to)?;
            copy_over(&e.path(), &to)?;
        } else {
            fs::copy(e.path(), &to)?;
        }
    }
    Ok(())
}

pub async fn validate_task(sandbox: &Sandbox, lt: &LoadedTask) -> TaskValidation {
    let cancel = CancellationToken::new();
    let mut v = TaskValidation { task_id: lt.task.id.clone(), baseline_fails: false, solution_passes: false, problems: vec![] };
    let ws = match sandbox.create_workspace(&format!("validate-{}", lt.task.id), &lt.fixture_dir()) {
        Ok(w) => w,
        Err(e) => {
            v.problems.push(format!("cannot create workspace: {e}"));
            return v;
        }
    };

    // Mutation tasks start from partial tests, so the baseline must be judged by the full verification.
    let baseline = run_checks(sandbox, &ws, lt, true, &cancel, &|_| {}).await;
    v.baseline_fails = baseline.failure.is_some();
    let baseline_spawn_err = baseline
        .compile
        .iter()
        .map(|c| &c.spawn_error)
        .chain(baseline.tests.iter().map(|t| &t.command.spawn_error))
        .flatten()
        .next()
        .cloned();
    if let Some(e) = baseline_spawn_err {
        v.problems.push(format!("baseline could not run: {e}"));
    } else if !v.baseline_fails {
        v.problems.push("the unmodified fixture already passes verification".into());
    }

    let sol = lt.solution_dir();
    if !sol.is_dir() {
        v.problems.push("missing solution/ directory with the reference solution".into());
    } else if let Err(e) = copy_over(&sol, ws.root()) {
        v.problems.push(format!("cannot apply solution: {e}"));
    } else {
        let after = run_checks(sandbox, &ws, lt, true, &cancel, &|_| {}).await;
        v.solution_passes = after.solved();
        if !v.solution_passes {
            let (why, out) = after.failure_feedback();
            v.problems.push(format!("the reference solution fails verification ({why}): {}", out.chars().take(600).collect::<String>()));
        }
        if let Verification::Mutation { mutants, .. } = &lt.task.verification {
            if after.mutants.len() != mutants.len() {
                v.problems.push("not every mutant was evaluated".into());
            }
        }
    }
    ws.destroy();
    v
}

/// Validate every task of a suite using a throw-away local sandbox. Requires the suite's
/// toolchain (if any) to be provided through `node_toolchain`.
pub async fn validate_suite(suite: &LoadedSuite, work_root: &Path, node_toolchain: Option<std::path::PathBuf>) -> Vec<TaskValidation> {
    let sandbox = Sandbox::new(ExecutionMode::Local, DockerImages::default(), work_root.to_path_buf(), node_toolchain);
    let mut out = Vec::new();
    for t in &suite.tasks {
        out.push(validate_task(&sandbox, t).await);
    }
    out
}
