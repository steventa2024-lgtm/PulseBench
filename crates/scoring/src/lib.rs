//! Transparent scoring.
//!
//! Every task yields six components in `0..=1`:
//!
//! | component   | meaning                                                                       |
//! |-------------|-------------------------------------------------------------------------------|
//! | correctness | 1 if verification fully passed, else 0                                        |
//! | tests       | fraction of tests passing (mutants killed, for "write tests" tasks)           |
//! | compile     | 1 if the compile/type-check step succeeded (absent if the task has none)      |
//! | efficiency  | `1 - (k-1)/max_attempts` when solved on attempt `k`, else 0                   |
//! | speed       | log-scaled generation tokens/s between a floor and a ceiling (absent if unknown) |
//! | reliability | 1 clean protocol JSON, 0.5 recovered from malformed output, 0 failed/timeout/truncated |
//!
//! A set of tasks (one task, one category, or a whole model) is scored by taking each component's
//! difficulty-weighted mean over the tasks where it applies, then
//! `score = Σ weight_c · component_c / Σ weight_c` over components that applied (weights of
//! components that did not apply are dropped and the rest renormalized).
//! The PulseBench Score is that value × 10 000.

use std::collections::BTreeMap;

use pulsebench_types::{
    Attempt, Category, CategoryScore, ComponentScores, Difficulty, FailureKind, FinishReason, ModelScore, ModelStats, ParseMode,
    ResourceSummary, ScoringConfig, TaskResult, TaskScore, TaskStatus,
};

pub fn difficulty_weight(d: Difficulty, cfg: &ScoringConfig) -> f64 {
    match d {
        Difficulty::Easy => cfg.difficulty_weights.easy,
        Difficulty::Medium => cfg.difficulty_weights.medium,
        Difficulty::Hard => cfg.difficulty_weights.hard,
    }
}

/// Fraction of verification satisfied by one attempt, `0..=1`.
pub fn test_fraction(attempt: &Attempt) -> f64 {
    let Some(t) = &attempt.tests else { return 0.0 };
    if !t.passed {
        // Partial credit from the reported counts, if available.
        return match (t.summary.total, t.summary.passed, t.summary.failed) {
            (Some(total), Some(passed), _) if total > 0 && attempt.mutants.is_empty() => (passed as f64 / total as f64).clamp(0.0, 1.0),
            (Some(total), None, Some(failed)) if total > 0 && attempt.mutants.is_empty() => {
                (total.saturating_sub(failed) as f64 / total as f64).clamp(0.0, 1.0)
            }
            _ => 0.0,
        };
    }
    if attempt.mutants.is_empty() {
        // Tests passed, but a failure like `NoTestsFound`/`TooFewTests` can still gate credit.
        return if matches!(attempt.failure, Some(FailureKind::NoTestsFound | FailureKind::TooFewTests)) { 0.0 } else { 1.0 };
    }
    let killed = attempt.mutants.iter().filter(|m| m.killed).count() as f64;
    let gate = if matches!(attempt.failure, Some(FailureKind::TooFewTests | FailureKind::NoTestsFound)) { 0.0 } else { 1.0 };
    gate * killed / attempt.mutants.len() as f64
}

fn attempt_reliability(a: &Attempt) -> f64 {
    if matches!(
        a.failure,
        Some(FailureKind::ProviderError | FailureKind::GenerationTimeout | FailureKind::Truncated | FailureKind::Cancelled)
    ) {
        return 0.0;
    }
    if let Some(g) = &a.generation {
        if g.finish_reason == Some(FinishReason::Length) {
            return 0.0;
        }
    } else {
        return 0.0;
    }
    match a.parse.as_ref().map(|p| p.mode) {
        Some(ParseMode::Json) => 1.0,
        Some(ParseMode::Recovered) => 0.5,
        _ => 0.0,
    }
}

pub fn speed_component(tps: f64, cfg: &ScoringConfig) -> f64 {
    let (lo, hi) = (cfg.speed_floor_tps.max(0.001), cfg.speed_ceiling_tps.max(cfg.speed_floor_tps.max(0.001) * 1.01));
    if tps <= lo {
        return 0.0;
    }
    ((tps / lo).ln() / (hi / lo).ln()).clamp(0.0, 1.0)
}

/// Component scores for one (non-skipped) task result.
pub fn task_components(task: &TaskResult, cfg: &ScoringConfig) -> ComponentScores {
    let attempts = &task.attempts;
    let last = attempts.last();
    let solved_at = attempts.iter().position(|a| a.solved).map(|i| i as u32 + 1);
    let max_attempts = task.max_attempts.max(attempts.len() as u32).max(1) as f64;
    let has_compile = task.baseline.as_ref().map(|b| b.compile.is_some()).unwrap_or(false) || attempts.iter().any(|a| a.compile.is_some());

    let correctness = Some(if task.status == TaskStatus::Passed { 1.0 } else { 0.0 });
    let tests = Some(last.map(test_fraction).unwrap_or(0.0));
    let compile = has_compile.then(|| match last.and_then(|a| a.compile.as_ref()) {
        Some(c) if c.success() => 1.0,
        _ => 0.0,
    });
    let efficiency = Some(match solved_at {
        Some(k) => 1.0 - (k as f64 - 1.0) / max_attempts,
        None => 0.0,
    });
    let rates: Vec<f64> =
        attempts.iter().filter_map(|a| a.generation.as_ref().and_then(|g| g.tokens_per_second)).filter(|t| *t > 0.0).collect();
    let speed = (!rates.is_empty()).then(|| speed_component(rates.iter().sum::<f64>() / rates.len() as f64, cfg));
    let reliability =
        Some(if attempts.is_empty() { 0.0 } else { attempts.iter().map(attempt_reliability).sum::<f64>() / attempts.len() as f64 });

    ComponentScores { correctness, tests, compile, efficiency, speed, reliability }
}

fn weights_vec(cfg: &ScoringConfig) -> [f64; 6] {
    let w = &cfg.weights;
    [w.correctness, w.tests, w.compile, w.efficiency, w.speed, w.reliability]
}

fn comps_vec(c: &ComponentScores) -> [Option<f64>; 6] {
    [c.correctness, c.tests, c.compile, c.efficiency, c.speed, c.reliability]
}

/// `Σ w·c / Σ w` over components that apply, `0..=1`. `None` if nothing applies.
pub fn combine(components: &ComponentScores, cfg: &ScoringConfig) -> Option<f64> {
    let (mut num, mut den) = (0.0, 0.0);
    for (w, c) in weights_vec(cfg).into_iter().zip(comps_vec(components)) {
        if let Some(c) = c {
            num += w.max(0.0) * c.clamp(0.0, 1.0);
            den += w.max(0.0);
        }
    }
    (den > 0.0).then(|| num / den)
}

pub fn score_task(task: &TaskResult, cfg: &ScoringConfig) -> Option<TaskScore> {
    if task.status == TaskStatus::Skipped {
        return None;
    }
    let components = task_components(task, cfg);
    let value = combine(&components, cfg).unwrap_or(0.0) * 100.0;
    Some(TaskScore { value, components, weight: difficulty_weight(task.difficulty, cfg) })
}

/// Difficulty-weighted mean of each component across tasks where it applies.
pub fn aggregate_components(scores: &[&TaskScore]) -> ComponentScores {
    fn mean(scores: &[&TaskScore], f: impl Fn(&ComponentScores) -> Option<f64>) -> Option<f64> {
        let (mut num, mut den) = (0.0, 0.0);
        for s in scores {
            if let Some(v) = f(&s.components) {
                num += s.weight * v;
                den += s.weight;
            }
        }
        (den > 0.0).then(|| num / den)
    }
    ComponentScores {
        correctness: mean(scores, |c| c.correctness),
        tests: mean(scores, |c| c.tests),
        compile: mean(scores, |c| c.compile),
        efficiency: mean(scores, |c| c.efficiency),
        speed: mean(scores, |c| c.speed),
        reliability: mean(scores, |c| c.reliability),
    }
}

/// Score a model from its task results. Returns `None` if no task was scorable.
pub fn score_model(tasks: &[TaskResult], cfg: &ScoringConfig) -> Option<ModelScore> {
    let scored: Vec<(&TaskResult, &TaskScore)> = tasks.iter().filter_map(|t| t.score.as_ref().map(|s| (t, s))).collect();
    if scored.is_empty() {
        return None;
    }
    let all: Vec<&TaskScore> = scored.iter().map(|(_, s)| *s).collect();
    let components = aggregate_components(&all);
    let value = combine(&components, cfg).unwrap_or(0.0);

    let mut by_cat: BTreeMap<Category, Vec<(&TaskResult, &TaskScore)>> = BTreeMap::new();
    for (t, s) in &scored {
        by_cat.entry(t.category).or_default().push((*t, *s));
    }
    let categories = by_cat
        .into_iter()
        .map(|(category, items)| {
            let s: Vec<&TaskScore> = items.iter().map(|(_, s)| *s).collect();
            let v = combine(&aggregate_components(&s), cfg).unwrap_or(0.0) * 100.0;
            CategoryScore {
                category,
                score: v,
                tasks: items.len() as u32,
                passed: items.iter().filter(|(t, _)| t.status == TaskStatus::Passed).count() as u32,
            }
        })
        .collect();

    Some(ModelScore {
        pulsebench_score: (value * 10_000.0).round().clamp(0.0, 10_000.0) as u32,
        components,
        categories,
        formula: cfg.formula.clone(),
    })
}

/// Aggregate statistics for a model.
pub fn model_stats(tasks: &[TaskResult], resources: ResourceSummary) -> ModelStats {
    let mut s = ModelStats { tasks_total: tasks.len() as u32, resources, ..Default::default() };
    let mut gen_secs = Vec::new();
    let mut tps = Vec::new();
    let mut ttft = Vec::new();
    for t in tasks {
        match t.status {
            TaskStatus::Passed => s.tasks_passed += 1,
            TaskStatus::Failed => s.tasks_failed += 1,
            TaskStatus::Skipped => s.tasks_skipped += 1,
            TaskStatus::Error => s.tasks_error += 1,
        }
        s.total_seconds += t.duration_ms as f64 / 1000.0;
        s.repair_attempts += t.attempts.len().saturating_sub(1) as u32;
        for a in &t.attempts {
            if let Some(g) = &a.generation {
                gen_secs.push(g.duration_ms as f64 / 1000.0);
                if let Some(r) = g.tokens_per_second {
                    tps.push(r);
                }
                if let Some(f) = g.time_to_first_token_ms {
                    ttft.push(f as f64);
                }
                s.total_prompt_tokens += g.prompt_tokens.unwrap_or(0) as u64;
                s.total_completion_tokens += g.completion_tokens.unwrap_or(0) as u64;
            }
            match a.failure {
                Some(FailureKind::CompileFailed) => s.compile_failures += 1,
                Some(FailureKind::TestsFailed | FailureKind::TestTimeout | FailureKind::MutantsSurvived) => s.runtime_failures += 1,
                Some(FailureKind::ProtocolFailure | FailureKind::NoValidChanges | FailureKind::Truncated) => s.protocol_failures += 1,
                _ => {}
            }
        }
    }
    let scored = tasks.iter().filter(|t| t.status != TaskStatus::Skipped).count();
    s.pass_rate = if scored > 0 { s.tasks_passed as f64 / scored as f64 } else { 0.0 };
    let avg = |v: &[f64]| (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64);
    s.avg_generation_seconds = avg(&gen_secs);
    s.avg_tokens_per_second = avg(&tps);
    s.avg_time_to_first_token_ms = avg(&ttft);
    s
}

/// Relative difference of `a` over `b` in percent (`None` when `b` is zero).
pub fn percent_change(a: f64, b: f64) -> Option<f64> {
    (b != 0.0).then(|| (a - b) / b * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use pulsebench_types::*;

    fn gen(tps: Option<f64>, finish: FinishReason) -> GenerationResult {
        GenerationResult {
            text: "{}".into(),
            reasoning: None,
            model: "m".into(),
            provider: "p".into(),
            prompt_tokens: Some(100),
            completion_tokens: Some(200),
            total_tokens: Some(300),
            duration_ms: 4000,
            time_to_first_token_ms: Some(500),
            tokens_per_second: tps,
            load_duration_ms: None,
            finish_reason: Some(finish),
            metric_source: Some(MetricSource::ProviderReported),
        }
    }

    fn cmd(ok: bool) -> CommandOutcome {
        CommandOutcome {
            command: "x".into(),
            exit_code: Some(if ok { 0 } else { 1 }),
            timed_out: false,
            cancelled: false,
            duration_ms: 10,
            stdout: String::new(),
            stderr: String::new(),
            truncated: false,
            spawn_error: None,
        }
    }

    fn attempt(mode: ParseMode, tests: Option<(bool, u32, u32)>, compile: Option<bool>, solved: bool, tps: Option<f64>) -> Attempt {
        Attempt {
            number: 1,
            prompt: PromptRecord { system: String::new(), user: String::new(), files: vec![] },
            generation: Some(gen(tps, FinishReason::Stop)),
            parse: Some(ParseReport { mode, notes: vec![], explanation: None }),
            changes: vec![],
            diff: String::new(),
            compile: compile.map(cmd),
            tests: tests.map(|(ok, total, passed)| TestOutcome {
                command: cmd(ok),
                summary: TestSummary { total: Some(total), passed: Some(passed), failed: Some(total - passed), skipped: None },
                passed: ok,
            }),
            mutants: vec![],
            resources: ResourceSummary::default(),
            failure: None,
            error: None,
            solved,
            started_at: Utc::now(),
            duration_ms: 5000,
        }
    }

    fn task(status: TaskStatus, difficulty: Difficulty, category: Category, attempts: Vec<Attempt>, has_compile: bool) -> TaskResult {
        TaskResult {
            id: "t".into(),
            title: "t".into(),
            category,
            language: Language::Python,
            difficulty,
            status,
            failure: None,
            message: None,
            max_attempts: 2,
            attempts,
            baseline: Some(Baseline { compile: has_compile.then(|| cmd(false)), tests: None, already_passing: false }),
            score: None,
            started_at: Utc::now(),
            duration_ms: 6000,
        }
    }

    fn scored(mut t: TaskResult, cfg: &ScoringConfig) -> TaskResult {
        t.score = score_task(&t, cfg);
        t
    }

    #[test]
    fn perfect_task_scores_full_marks_when_speed_is_at_ceiling() {
        let cfg = ScoringConfig::default();
        let t = task(
            TaskStatus::Passed,
            Difficulty::Easy,
            Category::Bugfix,
            vec![attempt(ParseMode::Json, Some((true, 5, 5)), Some(true), true, Some(100.0))],
            true,
        );
        let s = score_task(&t, &cfg).unwrap();
        assert!((s.value - 100.0).abs() < 1e-9, "{s:?}");
    }

    #[test]
    fn hand_computed_example() {
        // Failed task: 3/4 tests pass, compile ok, not solved, recovered output, speed 20 tok/s.
        let cfg = ScoringConfig::default();
        let t = task(
            TaskStatus::Failed,
            Difficulty::Easy,
            Category::Bugfix,
            vec![attempt(ParseMode::Recovered, Some((false, 4, 3)), Some(true), false, Some(20.0))],
            true,
        );
        let c = task_components(&t, &cfg);
        assert_eq!(c.correctness, Some(0.0));
        assert_eq!(c.tests, Some(0.75));
        assert_eq!(c.compile, Some(1.0));
        assert_eq!(c.efficiency, Some(0.0));
        let speed = (20.0f64 / 5.0).ln() / (100.0f64 / 5.0).ln();
        assert!((c.speed.unwrap() - speed).abs() < 1e-12);
        assert_eq!(c.reliability, Some(0.5));
        // (60*0 + 20*0.75 + 5*1 + 5*0 + 5*speed + 5*0.5) / 100
        let expect = (20.0 * 0.75 + 5.0 + 5.0 * speed + 2.5) / 100.0;
        assert!((combine(&c, &cfg).unwrap() - expect).abs() < 1e-12);
    }

    #[test]
    fn missing_speed_and_compile_renormalize_instead_of_scoring_zero() {
        let cfg = ScoringConfig::default();
        let t = task(
            TaskStatus::Passed,
            Difficulty::Easy,
            Category::Bugfix,
            vec![attempt(ParseMode::Json, Some((true, 2, 2)), None, true, None)],
            false,
        );
        let c = task_components(&t, &cfg);
        assert_eq!(c.speed, None);
        assert_eq!(c.compile, None);
        assert!((combine(&c, &cfg).unwrap() - 1.0).abs() < 1e-12, "a perfect task without speed/compile data still scores 100%");
    }

    #[test]
    fn protocol_failure_scores_zero_and_unreliable() {
        let cfg = ScoringConfig::default();
        let mut a = attempt(ParseMode::Failed, None, None, false, Some(30.0));
        a.failure = Some(FailureKind::ProtocolFailure);
        let t = task(TaskStatus::Failed, Difficulty::Easy, Category::Bugfix, vec![a], false);
        let c = task_components(&t, &cfg);
        assert_eq!(c.correctness, Some(0.0));
        assert_eq!(c.tests, Some(0.0));
        assert_eq!(c.reliability, Some(0.0));
    }

    #[test]
    fn truncated_output_is_unreliable() {
        let mut a = attempt(ParseMode::Json, Some((true, 1, 1)), None, true, Some(30.0));
        a.generation.as_mut().unwrap().finish_reason = Some(FinishReason::Length);
        assert_eq!(attempt_reliability(&a), 0.0);
    }

    #[test]
    fn speed_mapping_is_log_scaled_and_clamped() {
        let cfg = ScoringConfig::default();
        assert_eq!(speed_component(1.0, &cfg), 0.0);
        assert_eq!(speed_component(5.0, &cfg), 0.0);
        assert!((speed_component(100.0, &cfg) - 1.0).abs() < 1e-12);
        assert_eq!(speed_component(500.0, &cfg), 1.0);
        let mid = speed_component((5.0f64 * 100.0).sqrt(), &cfg);
        assert!((mid - 0.5).abs() < 1e-9);
    }

    #[test]
    fn repair_attempt_reduces_efficiency() {
        let cfg = ScoringConfig::default();
        let a1 = attempt(ParseMode::Json, Some((false, 2, 1)), None, false, Some(50.0));
        let mut a2 = attempt(ParseMode::Json, Some((true, 2, 2)), None, true, Some(50.0));
        a2.number = 2;
        let t = task(TaskStatus::Passed, Difficulty::Easy, Category::Bugfix, vec![a1, a2], false);
        // solved on attempt 2 of 2 attempts => 1 - 1/2
        assert_eq!(task_components(&t, &cfg).efficiency, Some(0.5));
    }

    #[test]
    fn model_score_weights_by_difficulty_and_builds_categories() {
        let cfg = ScoringConfig::default();
        let easy_pass = scored(
            task(
                TaskStatus::Passed,
                Difficulty::Easy,
                Category::Bugfix,
                vec![attempt(ParseMode::Json, Some((true, 3, 3)), None, true, Some(100.0))],
                false,
            ),
            &cfg,
        );
        let hard_fail = scored(
            task(
                TaskStatus::Failed,
                Difficulty::Hard,
                Category::Security,
                vec![attempt(ParseMode::Json, Some((false, 4, 0)), None, false, Some(100.0))],
                false,
            ),
            &cfg,
        );
        let skipped = {
            let mut t = task(TaskStatus::Skipped, Difficulty::Medium, Category::Testing, vec![], false);
            t.score = None;
            t
        };
        let m = score_model(&[easy_pass, hard_fail, skipped], &cfg).unwrap();
        // correctness = (1*1 + 3*0)/4 = 0.25 ; tests same ; efficiency same ; speed 1 ; reliability 1
        // score = (60*.25 + 20*.25 + 5*.25 + 5*1 + 5*1)/ (60+20+5+5+5) -> compile absent so den=95
        let expect: f64 = (60.0 * 0.25 + 20.0 * 0.25 + 5.0 * 0.25 + 5.0 + 5.0) / 95.0;
        assert_eq!(m.pulsebench_score, (expect * 10_000.0).round() as u32);
        assert_eq!(m.categories.len(), 2, "skipped category must not appear");
        let sec = m.categories.iter().find(|c| c.category == Category::Security).unwrap();
        assert_eq!(sec.passed, 0);
        assert!(sec.score < 30.0);
    }

    #[test]
    fn no_scorable_tasks_means_no_score() {
        assert!(score_model(&[], &ScoringConfig::default()).is_none());
    }

    #[test]
    fn stats_count_outcomes() {
        let cfg = ScoringConfig::default();
        let mut bad = attempt(ParseMode::Failed, None, None, false, None);
        bad.failure = Some(FailureKind::ProtocolFailure);
        let tasks = vec![
            scored(
                task(
                    TaskStatus::Passed,
                    Difficulty::Easy,
                    Category::Bugfix,
                    vec![attempt(ParseMode::Json, Some((true, 1, 1)), None, true, Some(40.0))],
                    false,
                ),
                &cfg,
            ),
            scored(task(TaskStatus::Failed, Difficulty::Easy, Category::Bugfix, vec![bad], false), &cfg),
            task(TaskStatus::Skipped, Difficulty::Easy, Category::Bugfix, vec![], false),
        ];
        let s = model_stats(&tasks, ResourceSummary::default());
        assert_eq!((s.tasks_passed, s.tasks_failed, s.tasks_skipped), (1, 1, 1));
        assert!((s.pass_rate - 0.5).abs() < 1e-12, "skipped tasks are excluded from the pass rate");
        assert_eq!(s.protocol_failures, 1);
        assert_eq!(s.avg_tokens_per_second, Some(40.0));
    }

    #[test]
    fn mutation_fraction_requires_gate() {
        let mut a = attempt(ParseMode::Json, Some((true, 3, 3)), None, false, None);
        a.mutants = vec![
            MutantOutcome { id: "m1".into(), description: String::new(), killed: true, exit_code: Some(1), duration_ms: 1 },
            MutantOutcome { id: "m2".into(), description: String::new(), killed: false, exit_code: Some(0), duration_ms: 1 },
        ];
        assert_eq!(test_fraction(&a), 0.5);
        a.tests.as_mut().unwrap().passed = false;
        assert_eq!(test_fraction(&a), 0.0, "tests failing on the correct implementation get no credit");
    }
}
