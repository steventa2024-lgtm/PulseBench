//! Compile, test and mutation verification of a workspace. Commands come only from the task
//! definition; nothing here ever executes model-supplied text.

use std::path::PathBuf;
use std::time::Duration;

use pulsebench_sandbox::{Sandbox, Workspace};
use pulsebench_types::{CommandOutcome, FailureKind, MutantOutcome, Phase, TestOutcome, Verification};
use tokio_util::sync::CancellationToken;

use crate::suite::LoadedTask;
use crate::testparse::parse_test_summary;

#[derive(Debug, Clone, Default)]
pub struct CheckOutcome {
    pub compile: Option<CommandOutcome>,
    pub tests: Option<TestOutcome>,
    pub mutants: Vec<MutantOutcome>,
    pub failure: Option<FailureKind>,
}

impl CheckOutcome {
    pub fn solved(&self) -> bool {
        self.failure.is_none()
    }

    pub fn cancelled(&self) -> bool {
        self.compile.as_ref().map(|c| c.cancelled).unwrap_or(false) || self.tests.as_ref().map(|t| t.command.cancelled).unwrap_or(false)
    }

    /// Short description and the command output most useful for a repair prompt.
    pub fn failure_feedback(&self) -> (String, String) {
        let join = |c: &CommandOutcome| format!("$ {}\n{}{}", c.command, c.stdout, c.stderr);
        match self.failure {
            Some(FailureKind::CompileFailed) => {
                ("the code does not compile / type-check".into(), self.compile.as_ref().map(join).unwrap_or_default())
            }
            Some(FailureKind::TestsFailed) | Some(FailureKind::TestTimeout) | Some(FailureKind::NoTestsFound) => {
                ("the tests did not pass".into(), self.tests.as_ref().map(|t| join(&t.command)).unwrap_or_default())
            }
            Some(FailureKind::TooFewTests) => {
                ("too few tests were written".into(), self.tests.as_ref().map(|t| join(&t.command)).unwrap_or_default())
            }
            Some(FailureKind::MutantsSurvived) => {
                let survived: Vec<String> = self.mutants.iter().filter(|m| !m.killed).map(|m| format!("- {}", m.description)).collect();
                (
                    "your tests pass on the correct code but did not detect these defects".into(),
                    format!("Undetected defects:\n{}", survived.join("\n")),
                )
            }
            _ => ("verification failed".into(), String::new()),
        }
    }
}

pub async fn run_checks(
    sandbox: &Sandbox,
    ws: &Workspace,
    lt: &LoadedTask,
    with_mutants: bool,
    cancel: &CancellationToken,
    on_phase: &(dyn Fn(Phase) + Send + Sync),
) -> CheckOutcome {
    let task = &lt.task;
    let timeout = Duration::from_secs(task.timeout_seconds as u64);
    let mut out = CheckOutcome::default();

    if let Some(cmd) = &task.compile_command {
        on_phase(Phase::Compiling);
        let c = sandbox.run_command(ws, cmd, timeout, cancel).await;
        let ok = c.success();
        out.compile = Some(c);
        if !ok {
            out.failure = Some(FailureKind::CompileFailed);
            return out;
        }
    }

    on_phase(Phase::Testing);
    let t = sandbox.run_command(ws, &task.test_command, timeout, cancel).await;
    let summary = parse_test_summary(&t.stdout, &t.stderr);
    let ran_nothing = summary.total == Some(0);
    let passed = t.success() && !ran_nothing;
    out.failure = if passed {
        None
    } else if t.timed_out {
        Some(FailureKind::TestTimeout)
    } else if t.success() && ran_nothing {
        Some(FailureKind::NoTestsFound)
    } else {
        Some(FailureKind::TestsFailed)
    };
    let total = summary.total;
    out.tests = Some(TestOutcome { command: t, summary, passed });
    if !passed || t_cancelled(&out) {
        return out;
    }

    if let (true, Verification::Mutation { min_tests, mutants }) = (with_mutants, &task.verification) {
        if let Some(total) = total {
            if total < *min_tests {
                out.failure = Some(FailureKind::TooFewTests);
                return out;
            }
        }
        on_phase(Phase::Verifying);
        for m in mutants {
            if cancel.is_cancelled() {
                break;
            }
            let overlay: Vec<(String, PathBuf)> = m.overlay.iter().map(|(target, src)| (target.clone(), lt.dir.join(src))).collect();
            let started = std::time::Instant::now();
            let backup = match ws.overlay(&overlay) {
                Ok(b) => b,
                Err(e) => {
                    out.failure = Some(FailureKind::TestsFailed);
                    out.mutants.push(MutantOutcome {
                        id: m.id.clone(),
                        description: format!("{} (could not apply mutant: {e})", m.description),
                        killed: false,
                        exit_code: None,
                        duration_ms: 0,
                    });
                    continue;
                }
            };
            // Compiled languages must rebuild the mutant first, otherwise stale output would be tested.
            let mut compile_failed = false;
            if let Some(cmd) = &task.compile_command {
                compile_failed = !sandbox.run_command(ws, cmd, timeout, cancel).await.success();
            }
            let r = if compile_failed { None } else { Some(sandbox.run_command(ws, &task.test_command, timeout, cancel).await) };
            let _ = ws.restore(backup);
            // A hang or failure on the broken implementation means the tests noticed something.
            // A mutant that does not even compile is a defect in the benchmark, never a "kill".
            let (killed, exit_code) = match &r {
                Some(r) => (!r.success() && !r.cancelled, r.exit_code),
                None => (false, None),
            };
            out.mutants.push(MutantOutcome {
                id: m.id.clone(),
                description: if compile_failed { format!("{} (mutant did not compile)", m.description) } else { m.description.clone() },
                killed,
                exit_code,
                duration_ms: started.elapsed().as_millis() as u32,
            });
        }
        if out.mutants.iter().any(|m| !m.killed) {
            out.failure = Some(FailureKind::MutantsSurvived);
        }
    }
    out
}

fn t_cancelled(o: &CheckOutcome) -> bool {
    o.cancelled()
}
