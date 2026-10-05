//! Human-readable exports: ranking, Markdown report and the shareable SVG card.

use pulsebench_scoring::percent_change;
use pulsebench_types::*;

/// Models ordered best-first: scored models by score, then pass rate, then speed; unscored last.
pub fn ranked(run: &RunResult) -> Vec<&ModelResult> {
    let mut v: Vec<&ModelResult> = run.models.iter().collect();
    v.sort_by(|a, b| {
        let sa = a.score.as_ref().map(|s| s.pulsebench_score);
        let sb = b.score.as_ref().map(|s| s.pulsebench_score);
        sb.cmp(&sa)
            .then(b.stats.pass_rate.partial_cmp(&a.stats.pass_rate).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.stats.total_seconds.partial_cmp(&b.stats.total_seconds).unwrap_or(std::cmp::Ordering::Equal))
    });
    v
}

pub fn thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn gpu_label(run: &RunResult) -> String {
    run.system.gpus.first().map(|g| g.name.clone()).unwrap_or_else(|| "no GPU detected".into())
}

pub fn ram_label(run: &RunResult) -> String {
    format!("{} GB RAM", (run.system.ram_total_mb as f64 / 1024.0).round() as u64)
}

pub fn format_duration(secs: f64) -> String {
    let s = secs.round() as u64;
    if s >= 3600 {
        format!("{}h {:02}m", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{}m {:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

fn opt(v: Option<f64>, digits: usize, unit: &str) -> String {
    v.map(|x| format!("{x:.digits$}{unit}")).unwrap_or_else(|| "n/a".into())
}

/// One evidence-based sentence about the winner.
pub fn summary_sentence(run: &RunResult) -> Option<String> {
    let r = ranked(run);
    let first = r.first()?;
    let score = first.score.as_ref()?;
    let scored_tasks = first.stats.tasks_total - first.stats.tasks_skipped;
    let mut s = format!(
        "On your {}, {} completed {}/{} tasks, scored {}",
        gpu_label(run),
        first.model.display_name,
        first.stats.tasks_passed,
        scored_tasks,
        thousands(score.pulsebench_score)
    );
    if let Some(t) = first.stats.avg_tokens_per_second {
        s.push_str(&format!(", averaged {t:.1} tokens/sec"));
    }
    if let Some(second) = r.get(1).and_then(|m| m.score.as_ref().map(|sc| (m, sc))) {
        if let Some(p) = percent_change(score.pulsebench_score as f64, second.1.pulsebench_score as f64) {
            s.push_str(&format!(" and outperformed {} by {:.1}%", second.0.model.display_name, p));
        }
    }
    s.push_str(" on this benchmark.");
    Some(s)
}

pub fn markdown_report(run: &RunResult) -> String {
    let mut m = String::new();
    let ranking = ranked(run);
    m.push_str("# PulseBench Report\n\n");
    if let Some(s) = summary_sentence(run) {
        m.push_str(&format!("> {s}\n\n"));
    }
    m.push_str("## System\n\n");
    m.push_str(&format!("- GPU: {}", gpu_label(run)));
    if let Some(g) = run.system.gpus.first() {
        if let Some(v) = g.vram_total_mb {
            m.push_str(&format!(" ({:.1} GB VRAM)", v as f64 / 1024.0));
        }
        if let Some(d) = &g.driver_version {
            m.push_str(&format!(", driver {d}"));
        }
    }
    m.push('\n');
    m.push_str(&format!("- CPU: {} ({} threads)\n", run.system.cpu_model, run.system.cpu_logical_cores));
    m.push_str(&format!("- Memory: {}\n", ram_label(run)));
    m.push_str(&format!("- OS: {} {} ({})\n\n", run.system.os_name, run.system.os_version, run.system.arch));

    m.push_str("## Benchmark\n\n");
    m.push_str(&format!(
        "- Suite: {} v{}{}\n",
        run.suite.name,
        run.suite.version,
        if run.suite.official { " (official)" } else { " (custom or modified)" }
    ));
    m.push_str(&format!("- Suite content hash: `{}`\n", run.suite.content_hash));
    m.push_str(&format!("- Run ID: `{}`  |  PulseBench {}  |  schema `{}`\n", run.id, run.app_version, run.schema));
    m.push_str(&format!("- Started: {}  |  Status: {:?}\n", run.created_at.format("%Y-%m-%d %H:%M UTC"), run.status));
    let g = &run.settings.generation;
    m.push_str(&format!(
        "- Generation: temperature {}, seed {}, context {} tokens, max output {} tokens, timeout {} s\n",
        g.temperature,
        g.seed.map(|s| s.to_string()).unwrap_or_else(|| "none".into()),
        g.context_tokens,
        g.max_output_tokens,
        g.timeout_seconds
    ));
    m.push_str(&format!(
        "- Standard settings: {}\n\n",
        if run.standard_settings { "yes" } else { "**no** (not directly comparable with default runs)" }
    ));

    if let Some(w) = ranking.first().filter(|w| w.score.is_some()) {
        m.push_str(&format!(
            "**Winner:** {} — **{}**, {:.0}% of tasks passed\n\n",
            w.model.display_name,
            thousands(w.score.as_ref().unwrap().pulsebench_score),
            w.stats.pass_rate * 100.0
        ));
    }

    m.push_str(
        "## Leaderboard\n\n| Rank | Model | Score | Pass | Tasks | Tok/s | Time | Peak VRAM |\n|---:|---|---:|---:|---:|---:|---:|---:|\n",
    );
    for (i, mr) in ranking.iter().enumerate() {
        m.push_str(&format!(
            "| {} | {} | {} | {:.0}% | {}/{} | {} | {} | {} |\n",
            i + 1,
            mr.model.display_name,
            mr.score.as_ref().map(|s| thousands(s.pulsebench_score)).unwrap_or_else(|| match mr.status {
                ModelRunStatus::Failed => "DNF".into(),
                _ => "—".into(),
            }),
            mr.stats.pass_rate * 100.0,
            mr.stats.tasks_passed,
            mr.stats.tasks_total - mr.stats.tasks_skipped,
            opt(mr.stats.avg_tokens_per_second, 1, ""),
            format_duration(mr.stats.total_seconds),
            mr.stats.resources.peak_vram_mb.map(|v| format!("{:.1} GB", v as f64 / 1024.0)).unwrap_or_else(|| "unavailable".into()),
        ));
    }
    m.push('\n');

    m.push_str("## Category scores (0–100)\n\n| Model |");
    for c in Category::ALL {
        m.push_str(&format!(" {} |", c.label()));
    }
    m.push_str("\n|---|");
    for _ in Category::ALL {
        m.push_str("---:|");
    }
    m.push('\n');
    for mr in &ranking {
        m.push_str(&format!("| {} |", mr.model.display_name));
        for c in Category::ALL {
            let v = mr.score.as_ref().and_then(|s| s.categories.iter().find(|x| x.category == c)).map(|x| format!("{:.0}", x.score));
            m.push_str(&format!(" {} |", v.unwrap_or_else(|| "—".into())));
        }
        m.push('\n');
    }
    m.push('\n');

    m.push_str("## Score components (0–100%)\n\n| Model | Correctness | Tests | Compile | Efficiency | Speed | Reliability |\n|---|---:|---:|---:|---:|---:|---:|\n");
    for mr in &ranking {
        if let Some(s) = &mr.score {
            let c = &s.components;
            let p = |v: Option<f64>| v.map(|x| format!("{:.0}", x * 100.0)).unwrap_or_else(|| "n/a".into());
            m.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                mr.model.display_name,
                p(c.correctness),
                p(c.tests),
                p(c.compile),
                p(c.efficiency),
                p(c.speed),
                p(c.reliability)
            ));
        }
    }
    let w = &run.settings.scoring.weights;
    m.push_str(&format!(
        "\nWeights ({}): correctness {}, tests {}, compile {}, efficiency {}, speed {}, reliability {}. Score = weighted mean × 10,000.\n\n",
        run.settings.scoring.formula, w.correctness, w.tests, w.compile, w.efficiency, w.speed, w.reliability
    ));

    m.push_str("## Task results\n\n");
    if let Some(first) = run.models.first() {
        m.push_str("| Task | Category | Difficulty |");
        for mr in &ranking {
            m.push_str(&format!(" {} |", mr.model.display_name));
        }
        m.push_str("\n|---|---|---|");
        for _ in &ranking {
            m.push_str(":---:|");
        }
        m.push('\n');
        let mut ids: Vec<(&str, &str, Category, Difficulty)> = vec![];
        for mr in &run.models {
            for t in &mr.tasks {
                if !ids.iter().any(|x| x.0 == t.id) {
                    ids.push((&t.id, &t.title, t.category, t.difficulty));
                }
            }
        }
        let _ = first;
        for (id, title, cat, diff) in ids {
            m.push_str(&format!("| {} — {} | {} | {:?} |", id, title, cat.label(), diff));
            for mr in &ranking {
                let cell = match mr.tasks.iter().find(|t| t.id == id) {
                    Some(t) => match t.status {
                        TaskStatus::Passed => "✅".to_string(),
                        TaskStatus::Failed => format!("❌ {}", t.failure.map(failure_label).unwrap_or("failed")),
                        TaskStatus::Skipped => "⏭ skipped".to_string(),
                        TaskStatus::Error => "⚠ error".to_string(),
                    },
                    None => "—".into(),
                };
                m.push_str(&format!(" {cell} |"));
            }
            m.push('\n');
        }
        m.push('\n');
    }

    if !run.notes.is_empty() {
        m.push_str("## Notes\n\n");
        for n in &run.notes {
            m.push_str(&format!("- {n}\n"));
        }
        m.push('\n');
    }
    m.push_str("---\nGenerated by PulseBench — real coding benchmarks on your hardware. Built by ZeroPulse AI.\n");
    m
}

pub fn failure_label(f: FailureKind) -> &'static str {
    match f {
        FailureKind::ProviderError => "provider error",
        FailureKind::GenerationTimeout => "generation timeout",
        FailureKind::Truncated => "truncated output",
        FailureKind::ProtocolFailure => "unparseable output",
        FailureKind::NoValidChanges => "no valid changes",
        FailureKind::CompileFailed => "compile failed",
        FailureKind::TestsFailed => "tests failed",
        FailureKind::TestTimeout => "test timeout",
        FailureKind::NoTestsFound => "no tests found",
        FailureKind::MutantsSurvived => "defects undetected",
        FailureKind::TooFewTests => "too few tests",
        FailureKind::Cancelled => "cancelled",
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// 1200×630 shareable card for one model. Pure SVG; no network, no account.
pub fn share_card_svg(run: &RunResult, model_key: &str) -> Option<String> {
    let m = run.models.iter().find(|m| m.key == model_key)?;
    let score = m.score.as_ref()?;
    let scored = m.stats.tasks_total - m.stats.tasks_skipped;
    let mut name = m.model.display_name.clone();
    if name.chars().count() > 30 {
        name = name.chars().take(29).collect::<String>() + "…";
    }
    let tps = m.stats.avg_tokens_per_second.map(|t| format!("{t:.1} tokens/sec")).unwrap_or_else(|| "tokens/sec n/a".into());
    let suite = format!("{} v{}", run.suite.name, run.suite.version).to_uppercase();
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630" role="img" aria-label="PulseBench result for {name_e}">
  <defs>
    <linearGradient id="glow" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#2b7fff" stop-opacity="0.22"/><stop offset="1" stop-color="#22d3ee" stop-opacity="0"/></linearGradient>
  </defs>
  <rect width="1200" height="630" fill="#070b14"/>
  <rect width="1200" height="630" fill="url(#glow)"/>
  <rect x="24" y="24" width="1152" height="582" fill="none" stroke="#1d2a44" stroke-width="2"/>
  <g font-family="'JetBrains Mono','SF Mono',Menlo,Consolas,monospace">
    <text x="72" y="104" font-size="26" letter-spacing="8" fill="#5aa2ff" font-weight="700">PULSEBENCH</text>
    <polyline points="1030,100 1060,100 1072,76 1088,120 1102,88 1112,100 1128,100" fill="none" stroke="#22d3ee" stroke-width="3"/>
    <text x="72" y="200" font-size="46" fill="#e6edf7" font-weight="600">{name_e}</text>
    <text x="72" y="400" font-size="170" fill="#ffffff" font-weight="700">{score_e}</text>
    <text x="76" y="446" font-size="24" letter-spacing="6" fill="#5aa2ff">{suite_e}</text>
    <text x="72" y="520" font-size="30" fill="#c4d0e4">{pass_pct:.0}% tasks passed  ·  {tps_e}</text>
    <text x="72" y="568" font-size="24" fill="#7f90ad">{gpu_e}  ·  {ram_e}</text>
    <text x="1128" y="568" font-size="22" fill="#5aa2ff" text-anchor="end">pulsebench.dev</text>
    <text x="1128" y="520" font-size="22" fill="#7f90ad" text-anchor="end">{passed}/{scored} tasks</text>
  </g>
</svg>
"##,
        name_e = esc(&name),
        score_e = thousands(score.pulsebench_score),
        suite_e = esc(&suite),
        pass_pct = m.stats.pass_rate * 100.0,
        tps_e = esc(&tps),
        gpu_e = esc(&gpu_label(run)),
        ram_e = esc(&ram_label(run)),
        passed = m.stats.tasks_passed,
        scored = scored,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulsebench_types::testing::sample_run;

    #[test]
    fn thousands_separator() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(8742), "8,742");
        assert_eq!(thousands(10000), "10,000");
    }

    #[test]
    fn markdown_has_winner_leaderboard_and_comparison_sentence() {
        let run = sample_run("r1", &[("slow:7b", 6000), ("fast:30b", 8742)]);
        let md = markdown_report(&run);
        assert!(md.contains("# PulseBench Report"));
        assert!(md.contains("**Winner:** fast:30b — **8,742**"));
        assert!(md.contains("| 1 | fast:30b | 8,742 |"));
        assert!(md.contains("outperformed slow:7b by 45.7%"), "{md}");
        assert!(md.contains("Test GPU"));
        assert!(md.contains("unavailable"), "missing VRAM must be shown as unavailable");
    }

    #[test]
    fn svg_card_is_escaped_and_contains_the_score() {
        let mut run = sample_run("r1", &[("a<b>&\"c", 8742)]);
        run.models[0].key = "ollama/x".into();
        let svg = share_card_svg(&run, "ollama/x").unwrap();
        assert!(svg.contains("8,742"));
        assert!(svg.contains("a&lt;b&gt;&amp;&quot;c"));
        assert!(!svg.contains("a<b>"));
        assert!(share_card_svg(&run, "nope").is_none());
    }

    #[test]
    fn unscored_models_rank_last() {
        let mut run = sample_run("r1", &[("a", 100), ("b", 200)]);
        run.models[1].score = None;
        run.models[1].status = ModelRunStatus::Failed;
        assert_eq!(ranked(&run)[0].model.id, "a");
        assert!(markdown_report(&run).contains("DNF"));
    }
}
