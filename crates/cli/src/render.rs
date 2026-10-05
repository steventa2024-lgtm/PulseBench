//! Plain-text rendering for the CLI.

use pulsebench_app::{ModelSelection, SuiteList};
use pulsebench_core::report::{format_duration, gpu_label, ram_label, ranked, thousands};
use pulsebench_types::*;

pub fn models(statuses: &[ProviderStatus]) -> String {
    let mut out = String::new();
    for s in statuses {
        let (dot, state) = match s.state {
            ConnectionState::Connected => ("●", "connected"),
            ConnectionState::Disabled => ("○", "disabled"),
            ConnectionState::Unreachable => ("○", "not detected"),
            ConnectionState::Error => ("!", "error"),
        };
        out.push_str(&format!("{dot} {} ({}) — {state}", s.name, s.base_url));
        if let Some(v) = &s.version {
            out.push_str(&format!(" v{v}"));
        }
        out.push('\n');
        if let Some(e) = &s.error {
            out.push_str(&format!("    {e}\n"));
        }
        for m in &s.models {
            let mut meta = vec![];
            if let Some(p) = &m.parameter_size {
                meta.push(p.clone());
            }
            if let Some(q) = &m.quantization {
                meta.push(q.clone());
            }
            if let Some(b) = m.size_bytes {
                meta.push(format!("{:.1} GB", b as f64 / 1e9));
            }
            if let Some(c) = m.context_length {
                meta.push(format!("{c} ctx"));
            }
            out.push_str(&format!("    {}{}\n", m.id, if meta.is_empty() { String::new() } else { format!("  [{}]", meta.join(", ")) }));
        }
        if s.state == ConnectionState::Connected && s.models.is_empty() {
            out.push_str("    no models installed\n");
        }
    }
    out
}

pub fn hardware(i: &SystemInfo) -> String {
    let mut out = String::new();
    out.push_str(&format!("OS       {} {} ({})\n", i.os_name, i.os_version, i.arch));
    out.push_str(&format!("CPU      {} ({} threads)\n", i.cpu_model, i.cpu_logical_cores));
    out.push_str(&format!(
        "RAM      {:.1} GB total, {:.1} GB available\n",
        i.ram_total_mb as f64 / 1024.0,
        i.ram_available_mb as f64 / 1024.0
    ));
    if i.gpus.is_empty() {
        out.push_str("GPU      none detected\n");
    }
    for g in &i.gpus {
        out.push_str(&format!(
            "GPU      {}  VRAM {}  driver {}  telemetry {}\n",
            g.name,
            g.vram_total_mb.map(|v| format!("{:.1} GB", v as f64 / 1024.0)).unwrap_or_else(|| "unavailable".into()),
            g.driver_version.clone().unwrap_or_else(|| "unknown".into()),
            if g.telemetry_available { "live" } else { "unavailable" }
        ));
    }
    for r in &i.runtimes {
        out.push_str(&format!(
            "{:<8} {}\n",
            r.name,
            if r.available { r.version.clone().unwrap_or_else(|| "available".into()) } else { "not found".into() }
        ));
    }
    out.push_str(&format!(
        "Docker   {}\n",
        match (i.docker.installed, i.docker.running) {
            (false, _) => "not installed".to_string(),
            (true, false) => "installed, daemon not reachable".to_string(),
            (true, true) => format!("running {}", i.docker.version.clone().unwrap_or_default()),
        }
    ));
    out
}

pub fn suites(list: &SuiteList) -> String {
    let mut out = String::new();
    for s in &list.suites {
        out.push_str(&format!(
            "{:<12} v{:<8} {:>2} tasks  {}{}\n    {}\n",
            s.id,
            s.version,
            s.task_count,
            if s.official { "official" } else { "custom" },
            s.estimated_minutes.as_ref().map(|m| format!("  ~{m} min")).unwrap_or_default(),
            s.description
        ));
    }
    for p in &list.problems {
        out.push_str(&format!("! invalid suite {}: {}\n", p.path, p.error));
    }
    out
}

pub fn history(runs: &[RunSummary]) -> String {
    if runs.is_empty() {
        return "No benchmark runs yet. Try `pulsebench run quick --model <model>`.\n".into();
    }
    let mut out = format!("{:<24} {:<16} {:<10} {:<11} {}\n", "RUN ID", "DATE", "SUITE", "STATUS", "RESULTS");
    for r in runs {
        let best = r.models.iter().filter_map(|m| m.score.map(|s| (m, s))).max_by_key(|(_, s)| *s);
        out.push_str(&format!(
            "{:<24} {:<16} {:<10} {:<11} {}\n",
            r.id,
            r.created_at.format("%Y-%m-%d %H:%M"),
            format!("{} v{}", r.suite_id, r.suite_version),
            format!("{:?}", r.status).to_lowercase(),
            match best {
                Some((m, s)) => format!("{} models, best {} ({})", r.models.len(), m.display_name, thousands(s)),
                None => format!("{} models", r.models.len()),
            }
        ));
    }
    out
}

pub fn leaderboard(run: &RunResult) -> String {
    let mut out = format!("{} v{} — {}\n", run.suite.name.to_uppercase(), run.suite.version, gpu_label(run));
    out.push_str(&format!("{:<5}{:<34}{:>7}{:>7}{:>9}{:>9}\n", "RANK", "MODEL", "SCORE", "PASS", "TOK/S", "TIME"));
    out.push_str(&"─".repeat(71));
    out.push('\n');
    for (i, m) in ranked(run).iter().enumerate() {
        let name: String = m.model.display_name.chars().take(32).collect();
        out.push_str(&format!(
            "{:<5}{:<34}{:>7}{:>7}{:>9}{:>9}\n",
            i + 1,
            name,
            m.score.as_ref().map(|s| s.pulsebench_score.to_string()).unwrap_or_else(|| "DNF".into()),
            format!("{:.0}%", m.stats.pass_rate * 100.0),
            m.stats.avg_tokens_per_second.map(|t| format!("{t:.1}")).unwrap_or_else(|| "n/a".into()),
            format_duration(m.stats.total_seconds),
        ));
        if let Some(e) = &m.error {
            out.push_str(&format!("     ! {e}\n"));
        }
    }
    out.push('\n');
    if let Some(s) = pulsebench_core::report::summary_sentence(run) {
        out.push_str(&s);
        out.push('\n');
    }
    out.push_str(&format!(
        "{} · {:?}{}\n",
        ram_label(run),
        run.status,
        if run.standard_settings { "" } else { " · non-standard settings" }
    ));
    out
}

pub fn event_line(ev: &RunEvent) -> Option<String> {
    match ev {
        RunEvent::Log { entry, .. } => Some(format!(
            "{} {:<10} {}{}",
            entry.ts.with_timezone(&chrono::Local).format("%H:%M:%S"),
            entry.scope,
            match entry.level {
                LogLevel::Error => "ERROR ",
                LogLevel::Warn => "WARN  ",
                LogLevel::Info => "",
            },
            entry.message
        )),
        _ => None,
    }
}

/// Map `--model` arguments onto installed models.
pub fn resolve_models(statuses: &[ProviderStatus], wanted: &[String]) -> Result<Vec<ModelSelection>, String> {
    let mut out = Vec::new();
    for w in wanted {
        let w = w.trim();
        // `provider/model` form when the prefix names a provider.
        let (prefix, rest) = w.split_once('/').map(|(a, b)| (Some(a), b)).unwrap_or((None, w));
        let scoped: Vec<(&ProviderStatus, &ModelInfo)> = statuses
            .iter()
            .filter(|s| prefix.map(|p| s.provider_id == p).unwrap_or(false))
            .flat_map(|s| s.models.iter().map(move |m| (s, m)))
            .filter(|(_, m)| m.id == rest)
            .collect();
        let all: Vec<(&ProviderStatus, &ModelInfo)> = statuses.iter().flat_map(|s| s.models.iter().map(move |m| (s, m))).collect();
        let exact: Vec<_> = if !scoped.is_empty() { scoped } else { all.iter().copied().filter(|(_, m)| m.id == w).collect() };
        let candidates = if exact.is_empty() {
            let lw = w.to_lowercase();
            all.iter().copied().filter(|(_, m)| m.id.to_lowercase().contains(&lw)).collect()
        } else {
            exact
        };
        match candidates.as_slice() {
            [(s, m)] => out.push(ModelSelection { provider_id: s.provider_id.clone(), model_id: m.id.clone() }),
            [] => {
                let unreachable: Vec<String> = statuses
                    .iter()
                    .filter(|s| s.state != ConnectionState::Connected && s.state != ConnectionState::Disabled)
                    .map(|s| format!("{} ({})", s.name, s.error.clone().unwrap_or_default()))
                    .collect();
                return Err(format!(
                    "model '{w}' not found on any connected provider.{}",
                    if unreachable.is_empty() { String::new() } else { format!(" Unreachable providers: {}", unreachable.join("; ")) }
                ));
            }
            many => {
                let names: Vec<String> = many.iter().map(|(s, m)| format!("{}/{}", s.provider_id, m.id)).collect();
                return Err(format!("'{w}' is ambiguous: {}", names.join(", ")));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(id: &str, models: &[&str]) -> ProviderStatus {
        ProviderStatus {
            provider_id: id.into(),
            name: id.into(),
            kind: ProviderKind::Ollama,
            base_url: "http://x".into(),
            state: ConnectionState::Connected,
            version: None,
            error: None,
            latency_ms: None,
            models: models
                .iter()
                .map(|m| {
                    let mut i = pulsebench_types::testing::sample_model(m);
                    i.provider_id = id.into();
                    i
                })
                .collect(),
        }
    }

    #[test]
    fn resolves_exact_prefixed_and_partial_names() {
        let st = vec![status("ollama", &["qwen3-coder:30b", "qwen2.5-coder:7b"]), status("lmstudio", &["qwen3-coder:30b", "codestral"])];
        assert_eq!(resolve_models(&st, &["qwen2.5-coder:7b".into()]).unwrap()[0].provider_id, "ollama");
        assert_eq!(resolve_models(&st, &["lmstudio/qwen3-coder:30b".into()]).unwrap()[0].provider_id, "lmstudio");
        assert_eq!(resolve_models(&st, &["codestral".into()]).unwrap()[0].model_id, "codestral");
        assert_eq!(resolve_models(&st, &["CODEST".into()]).unwrap()[0].model_id, "codestral");
        assert!(resolve_models(&st, &["qwen3-coder:30b".into()]).unwrap_err().contains("ambiguous"));
        assert!(resolve_models(&st, &["nope".into()]).unwrap_err().contains("not found"));
    }

    #[test]
    fn leaderboard_lists_models_best_first() {
        let run = pulsebench_types::testing::sample_run("r", &[("a", 100), ("b", 9000)]);
        let text = leaderboard(&run);
        assert!(text.find("9000").unwrap() < text.find("100").unwrap());
        assert!(text.contains("RANK"));
    }
}
