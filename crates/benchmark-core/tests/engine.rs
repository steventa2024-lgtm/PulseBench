//! End-to-end runs of the real engine (real workspaces, real interpreters, real test execution)
//! against the shipped Quick suite, with scripted providers standing in for model servers.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pulsebench_core::{discover_suites, run_benchmark, BroadcastSink, LoadedSuite, RunEnv, RunRequest};
use pulsebench_providers::testing::ScriptedProvider;
use pulsebench_providers::{ProviderError, ProviderRegistry};
use pulsebench_types::testing::sample_system;
use pulsebench_types::*;
use tokio_util::sync::CancellationToken;

fn quick_suite() -> Arc<LoadedSuite> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks");
    let (suites, errors) = discover_suites(&[root]);
    assert!(errors.is_empty(), "{errors:?}");
    Arc::new(suites.into_iter().find(|s| s.manifest.id == "quick").unwrap())
}

/// title -> JSON response containing the reference solution for that task.
fn oracle_answers(suite: &LoadedSuite) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for t in &suite.tasks {
        let mut changes = vec![];
        for path in t.task.editable() {
            let p = t.solution_dir().join(path);
            if let Ok(content) = std::fs::read_to_string(&p) {
                changes.push(serde_json::json!({ "path": path, "content": content }));
            }
        }
        out.insert(t.task.title.clone(), serde_json::json!({ "changes": changes, "explanation": "reference" }).to_string());
    }
    out
}

fn task_title(prompt: &str) -> String {
    prompt.lines().next().unwrap().trim_start_matches("# Task: ").to_string()
}

fn env(providers: ProviderRegistry, sink: Arc<BroadcastSink>, work: &std::path::Path) -> RunEnv {
    RunEnv {
        providers,
        work_root: work.to_path_buf(),
        toolchain_root: std::env::temp_dir().join("pulsebench-test-cache"),
        sampler: None,
        system: sample_system(),
        sink,
        checkpoint: None,
    }
}

fn model(provider: &str, id: &str) -> ModelInfo {
    let mut m = pulsebench_types::testing::sample_model(id);
    m.provider_id = provider.into();
    m
}

fn request(suite: &Arc<LoadedSuite>, models: Vec<ModelInfo>) -> RunRequest {
    RunRequest {
        run_id: "run-test".into(),
        suite: suite.clone(),
        models,
        task_ids: None,
        settings: RunSettings::default(),
        keep_workspaces: false,
    }
}

fn dir_is_empty(p: &std::path::Path) -> bool {
    !p.exists() || std::fs::read_dir(p).map(|mut d| d.next().is_none()).unwrap_or(true)
}

#[tokio::test(flavor = "multi_thread")]
async fn two_models_get_identical_prompts_and_independent_scores() {
    let suite = quick_suite();
    let answers = oracle_answers(&suite);

    // Model "good" returns the reference solution (clean JSON); "fenced" returns the same wrapped in
    // markdown and prose; "bad" returns nonsense.
    let a1 = answers.clone();
    let good = ScriptedProvider::new("good", &["oracle"], Box::new(move |req, _| Ok(a1[&task_title(&req.prompt)].clone())));
    let a2 = answers.clone();
    let fenced = ScriptedProvider::new(
        "fenced",
        &["chatty"],
        Box::new(move |req, _| Ok(format!("Sure! Here is the fix:\n```json\n{}\n```\nHope that helps.", a2[&task_title(&req.prompt)]))),
    );
    let bad = ScriptedProvider::new("bad", &["rubbish"], Box::new(|_, _| Ok("I would rather not.".into())));
    let (good, fenced, bad) = (Arc::new(good), Arc::new(fenced), Arc::new(bad));

    let mut reg = ProviderRegistry::default();
    reg.insert(good.clone());
    reg.insert(fenced.clone());
    reg.insert(bad.clone());

    let work = tempfile::tempdir().unwrap();
    let sink = Arc::new(BroadcastSink::new(4096));
    let mut rx = sink.subscribe();
    let e = env(reg, sink, work.path());
    let run = run_benchmark(
        &e,
        request(&suite, vec![model("good", "oracle"), model("fenced", "chatty"), model("bad", "rubbish")]),
        CancellationToken::new(),
    )
    .await;

    assert_eq!(run.status, RunStatus::Completed);
    assert_eq!(run.schema, RESULT_SCHEMA);
    assert!(run.standard_settings);
    assert!(run.suite.official, "the shipped suite matches its lock file");
    let by = |k: &str| run.models.iter().find(|m| m.key == k).unwrap();
    let (g, f, b) = (by("good/oracle"), by("fenced/chatty"), by("bad/rubbish"));

    // Everything solvable was solved by the oracle; the nonsense model solved nothing.
    assert_eq!(
        (g.stats.tasks_passed, g.stats.tasks_total),
        (5, 5),
        "{:#?}",
        g.tasks.iter().map(|t| (&t.id, t.status, t.failure, &t.message)).collect::<Vec<_>>()
    );
    assert_eq!(f.stats.tasks_passed, 5);
    assert_eq!(b.stats.tasks_passed, 0);
    assert!(b.tasks.iter().all(|t| t.failure == Some(FailureKind::ProtocolFailure)));

    // Independently computed scores that rank sensibly: clean JSON > recovered JSON > garbage.
    let (gs, fs, bs) = (
        g.score.as_ref().unwrap().pulsebench_score,
        f.score.as_ref().unwrap().pulsebench_score,
        b.score.as_ref().unwrap().pulsebench_score,
    );
    assert!(gs > fs && fs > bs, "scores {gs} {fs} {bs}");
    assert!(g.score.as_ref().unwrap().components.reliability.unwrap() > f.score.as_ref().unwrap().components.reliability.unwrap());
    assert!(f.tasks.iter().all(|t| t.attempts[0].parse.as_ref().unwrap().mode == ParseMode::Recovered));

    // Fairness: every model saw byte-identical system and user prompts for every task.
    let prompts = |p: &ScriptedProvider| -> Vec<(String, String)> {
        p.seen.lock().unwrap().iter().map(|r| (r.system.clone(), r.prompt.clone())).collect()
    };
    assert_eq!(prompts(&good), prompts(&fenced));
    assert_eq!(prompts(&good), prompts(&bad));
    assert_eq!(good.seen.lock().unwrap()[0].settings, RunSettings::default().generation);

    // Task details are recorded: diff, test output, parsed changes.
    let t = &g.tasks[0];
    let a = &t.attempts[0];
    assert!(a.diff.contains("+++ b/inventory.py"), "{}", a.diff);
    assert!(a.tests.as_ref().unwrap().command.stderr.contains("Ran 8 tests"));
    assert_eq!(a.changes[0].status, ChangeStatus::Applied);
    assert!(!t.baseline.as_ref().unwrap().tests.as_ref().unwrap().passed);

    // The mutation task was verified against every mutant.
    let tt = g.tasks.iter().find(|t| t.id == "py-test-001").unwrap();
    assert_eq!(tt.attempts[0].mutants.len(), 6);
    assert!(tt.attempts[0].mutants.iter().all(|m| m.killed));

    // Logs and events were produced.
    assert!(run.log.iter().any(|l| l.scope == "GENERATION"));
    assert!(run.log.iter().any(|l| l.scope == "SCORE"));
    let mut kinds = std::collections::BTreeSet::new();
    while let Ok(ev) = rx.try_recv() {
        kinds.insert(match ev {
            RunEvent::RunStarted { .. } => "runStarted",
            RunEvent::ModelStarted { .. } => "modelStarted",
            RunEvent::TaskStarted { .. } => "taskStarted",
            RunEvent::PhaseChanged { .. } => "phase",
            RunEvent::Generation { .. } => "generation",
            RunEvent::TaskFinished { .. } => "taskFinished",
            RunEvent::ModelFinished { .. } => "modelFinished",
            RunEvent::RunFinished { .. } => "runFinished",
            RunEvent::Log { .. } => "log",
            RunEvent::Telemetry { .. } => "telemetry",
        });
    }
    for k in ["runStarted", "modelStarted", "taskStarted", "phase", "taskFinished", "modelFinished", "runFinished", "log"] {
        assert!(kinds.contains(k), "missing event {k}: {kinds:?}");
    }

    // Isolation: no workspace is left behind and the fixtures were never touched.
    assert!(dir_is_empty(&work.path().join("run-test")));
    let fx = suite.tasks[0].fixture_dir().join("inventory.py");
    assert!(std::fs::read_to_string(fx).unwrap().contains("item[\"quantity\"] < threshold"));

    // The run document round-trips through JSON and validates against the published schema.
    let json = serde_json::to_value(&run).unwrap();
    let schema = pulsebench_types::schema::result_schema();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<String> = validator.iter_errors(&json).map(|e| e.to_string()).take(5).collect();
    assert!(errors.is_empty(), "result does not match its schema: {errors:?}");
    let back: RunResult = serde_json::from_value(json).unwrap();
    assert_eq!(back, run);
}

#[tokio::test(flavor = "multi_thread")]
async fn path_escapes_and_foreign_files_are_rejected_and_the_fixture_stays_clean() {
    let suite = quick_suite();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("pwned.txt");
    let t = target.display().to_string();
    let answers = oracle_answers(&suite);
    let evil = ScriptedProvider::new(
        "evil",
        &["m"],
        Box::new(move |req, _| {
            let honest: serde_json::Value = serde_json::from_str(&answers[&task_title(&req.prompt)]).unwrap();
            let mut changes = honest["changes"].as_array().unwrap().clone();
            changes.push(serde_json::json!({"path": "../../pwned.txt", "content": "x"}));
            changes.push(serde_json::json!({"path": t, "content": "x"}));
            changes.push(serde_json::json!({"path": "node_modules/evil/index.js", "content": "x"}));
            changes.push(serde_json::json!({"path": "test_inventory.py", "content": "import unittest\nclass T(unittest.TestCase):\n    def test_x(self): pass\n"}));
            Ok(serde_json::json!({ "changes": changes }).to_string())
        }),
    );
    let mut reg = ProviderRegistry::default();
    reg.insert(Arc::new(evil));
    let work = tempfile::tempdir().unwrap();
    let e = env(reg, Arc::new(BroadcastSink::new(64)), work.path());
    let mut req = request(&suite, vec![model("evil", "m")]);
    req.task_ids = Some(vec!["py-bug-001".into()]);
    let run = run_benchmark(&e, req, CancellationToken::new()).await;
    let task = &run.models[0].tasks[0];
    let rejected = task.attempts[0].changes.iter().filter(|c| c.status == ChangeStatus::Rejected).count();
    assert_eq!(rejected, 4, "{:?}", task.attempts[0].changes);
    assert!(!target.exists());
    assert!(!work.path().join("pwned.txt").exists());
    // The model could not replace the (read-only context) tests, so the real tests still ran and passed.
    assert_eq!(task.status, TaskStatus::Passed);
    assert!(task.attempts[0].tests.as_ref().unwrap().command.stderr.contains("Ran 8 tests"));
    assert!(!run.standard_settings, "a task subset is not a standard run");
}

#[tokio::test(flavor = "multi_thread")]
async fn repair_attempts_receive_test_output_and_reduce_efficiency() {
    // Run a suite task whose max_attempts is 2 by copying the quick suite and editing one task.
    let src = quick_suite();
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&src.dir, dir.path());
    let tj = dir.path().join("tasks/py-bug-001/task.json");
    let text = std::fs::read_to_string(&tj).unwrap().replace("\"maxAttempts\": 1", "\"maxAttempts\": 2");
    std::fs::write(&tj, text).unwrap();
    std::fs::remove_file(dir.path().join("suite.lock.json")).unwrap();
    let suite = Arc::new(pulsebench_core::load_suite(dir.path()).unwrap());
    assert!(!suite.is_verified_official(), "a modified copy must not be reported as official");

    let answers = oracle_answers(&suite);
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let c = calls.clone();
    let p = ScriptedProvider::new(
        "p",
        &["m"],
        Box::new(move |req, n| {
            c.lock().unwrap().push(req.prompt.clone());
            if n == 1 {
                // First attempt: a plausible but wrong change.
                Ok(r#"{"changes":[{"path":"inventory.py","content":"def low_stock(items, threshold):\n    return []\n\ndef restock_plan(items, target):\n    return {}\n\ndef total_value(items):\n    return 0\n"}]}"#.into())
            } else {
                Ok(answers[&task_title(&req.prompt)].clone())
            }
        }),
    );
    let mut reg = ProviderRegistry::default();
    reg.insert(Arc::new(p));
    let work = tempfile::tempdir().unwrap();
    let e = env(reg, Arc::new(BroadcastSink::new(64)), work.path());
    let mut req = request(&suite, vec![model("p", "m")]);
    req.task_ids = Some(vec!["py-bug-001".into()]);
    let run = run_benchmark(&e, req, CancellationToken::new()).await;
    let t = &run.models[0].tasks[0];
    assert_eq!(t.status, TaskStatus::Passed);
    assert_eq!(t.attempts.len(), 2);
    assert!(!t.attempts[0].solved && t.attempts[1].solved);
    let second = calls.lock().unwrap()[1].clone();
    assert!(second.contains("Attempt 1 failed"), "{second}");
    assert!(second.contains("FAILED"), "the repair prompt must include the real test output");
    assert_eq!(t.score.as_ref().unwrap().components.efficiency, Some(0.5));
    assert_eq!(run.models[0].stats.repair_attempts, 1);
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), &to).unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn generation_errors_are_recorded_not_swallowed() {
    let suite = quick_suite();
    let timeouts = ScriptedProvider::new("slow", &["m"], Box::new(|_, _| Err(ProviderError::Timeout(5))));
    let broken = ScriptedProvider::new(
        "down",
        &["m"],
        Box::new(|_, _| Err(ProviderError::Unreachable { url: "http://x".into(), detail: "refused".into() })),
    );
    let mut reg = ProviderRegistry::default();
    reg.insert(Arc::new(timeouts));
    reg.insert(Arc::new(broken));
    let work = tempfile::tempdir().unwrap();
    let e = env(reg, Arc::new(BroadcastSink::new(64)), work.path());
    let run = run_benchmark(&e, request(&suite, vec![model("slow", "m"), model("down", "m")]), CancellationToken::new()).await;

    let slow = &run.models[0];
    assert!(slow.tasks.iter().all(|t| t.status == TaskStatus::Failed && t.failure == Some(FailureKind::GenerationTimeout)));
    assert_eq!(slow.status, ModelRunStatus::Completed, "timeouts are the model's problem, the run goes on");

    let down = &run.models[1];
    assert_eq!(down.tasks.len(), 3, "abandoned after 3 consecutive provider errors");
    assert!(down.tasks.iter().all(|t| t.status == TaskStatus::Error));
    assert_eq!(down.status, ModelRunStatus::Failed);
    assert!(down.error.as_ref().unwrap().contains("consecutive"));
    assert!(run.log.iter().any(|l| l.level == LogLevel::Error));
}

#[tokio::test(flavor = "multi_thread")]
async fn cancellation_stops_promptly_keeps_finished_tasks_and_leaves_no_workspaces() {
    let suite = quick_suite();
    let answers = oracle_answers(&suite);
    let p =
        Arc::new(ScriptedProvider::new("p", &["m"], Box::new(move |req, _| Ok(answers[&task_title(&req.prompt)].clone()))).with_delay(700));
    let mut reg = ProviderRegistry::default();
    reg.insert(p);
    let work = tempfile::tempdir().unwrap();
    let e = env(reg, Arc::new(BroadcastSink::new(64)), work.path());
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    // Let the first task finish (generation 0.7 s + checks), then cancel during a later one.
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(2600)).await;
        c2.cancel();
    });
    let started = std::time::Instant::now();
    let run = run_benchmark(&e, request(&suite, vec![model("p", "m")]), cancel).await;
    assert!(started.elapsed() < Duration::from_secs(20), "cancel must not wait for the whole suite");
    assert_eq!(run.status, RunStatus::Cancelled);
    assert_eq!(run.models[0].status, ModelRunStatus::Cancelled);
    let done = run.models[0].tasks.len();
    assert!((1..5).contains(&done), "some but not all tasks finished: {done}");
    assert!(run.models[0].tasks.iter().all(|t| t.status == TaskStatus::Passed), "finished tasks keep their real results");
    assert!(run.models[0].score.is_some());
    assert!(dir_is_empty(&work.path().join("run-test")));
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_runtime_skips_tasks_honestly() {
    let suite = quick_suite();
    let p = ScriptedProvider::new("p", &["m"], Box::new(|_, _| Ok("{}".into())));
    let mut reg = ProviderRegistry::default();
    reg.insert(Arc::new(p));
    let work = tempfile::tempdir().unwrap();
    let e = env(reg, Arc::new(BroadcastSink::new(64)), work.path());
    // Docker mode with no reachable daemon (true in CI and in this sandbox): tasks are skipped, not faked.
    let docker = pulsebench_sandbox::runtime::detect_docker().await;
    if docker.running {
        return; // a real daemon is available; nothing to assert here
    }
    let mut req = request(&suite, vec![model("p", "m")]);
    req.settings.execution = ExecutionMode::Docker;
    let run = run_benchmark(&e, req, CancellationToken::new()).await;
    let m = &run.models[0];
    assert!(m.tasks.iter().all(|t| t.status == TaskStatus::Skipped && t.message.as_ref().unwrap().contains("Docker")));
    assert!(m.score.is_none(), "nothing scorable means no score, not a zero");
    assert_eq!(m.stats.tasks_skipped, 5);
}
