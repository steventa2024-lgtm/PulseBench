//! Validates the official benchmark suites that ship with the repository:
//! * every suite loads and matches its committed lock file (no silent changes without a version bump),
//! * every task's untouched fixture FAILS verification and its reference solution PASSES it.
//!
//! Needs `python3`, `node` and (first run only) network access for `npm ci` of the toolchain.

use std::path::PathBuf;

use pulsebench_core::{discover_suites, validate::validate_suite, LockStatus};
use tokio_util::sync::CancellationToken;

fn benchmarks_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks")
}

fn cache_dir() -> PathBuf {
    std::env::temp_dir().join("pulsebench-test-cache")
}

#[test]
fn official_suites_match_their_lock_files() {
    let (suites, errors) = discover_suites(&[benchmarks_dir()]);
    assert!(errors.is_empty(), "invalid suites: {errors:?}");
    assert!(!suites.is_empty());
    for s in &suites {
        assert!(s.manifest.official, "{} should be official", s.manifest.id);
        assert_eq!(
            s.lock,
            LockStatus::Verified,
            "suite '{}' changed without updating suite.lock.json. If the change is intentional, bump `version` in \
             suite.json and run `pulsebench suite lock benchmarks/{}`",
            s.manifest.id,
            s.manifest.id
        );
    }
}

#[test]
fn official_suites_have_enough_tasks_in_every_required_language() {
    let (suites, _) = discover_suites(&[benchmarks_dir()]);
    let total: usize = suites.iter().map(|s| s.tasks.len()).sum();
    assert!(total >= 10, "the MVP requires at least 10 tasks, found {total}");
    let mut langs = std::collections::BTreeSet::new();
    for s in &suites {
        for t in &s.tasks {
            langs.insert(format!("{:?}", t.task.language));
        }
    }
    for needed in ["Python", "Javascript", "Typescript", "React"] {
        assert!(langs.contains(needed), "no {needed} task found");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn every_task_fails_untouched_and_passes_with_its_reference_solution() {
    let (suites, _) = discover_suites(&[benchmarks_dir()]);
    let work = tempfile::tempdir().unwrap();
    let mut failures = Vec::new();
    for suite in &suites {
        let tc = match &suite.manifest.toolchain.node {
            Some(spec) => Some(
                pulsebench_sandbox::toolchain::prepare_node_toolchain(
                    &cache_dir(),
                    spec,
                    suite.node_lockfile.as_deref(),
                    &CancellationToken::new(),
                    &|m| eprintln!("{m}"),
                )
                .await
                .expect("toolchain install"),
            ),
            None => None,
        };
        for v in validate_suite(suite, work.path(), tc).await {
            eprintln!("{}/{}: baselineFails={} solutionPasses={}", suite.manifest.id, v.task_id, v.baseline_fails, v.solution_passes);
            if !v.ok() {
                failures.push(format!("{}/{}: {:?}", suite.manifest.id, v.task_id, v.problems));
            }
        }
    }
    assert!(failures.is_empty(), "task validation failures:\n{}", failures.join("\n"));
}
