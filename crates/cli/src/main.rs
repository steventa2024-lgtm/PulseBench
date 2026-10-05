//! `pulsebench` — the command-line interface. All behaviour lives in `pulsebench-app` and
//! `pulsebench-core`, shared with the desktop application.

mod render;

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use pulsebench_app::{App, AppPaths, ExportFormat, StartRunRequest};
use pulsebench_types::{ConnectionState, RunEvent, RunSettings, RunStatus};

#[derive(Parser)]
#[command(name = "pulsebench", version, about = "Real coding benchmarks. On your hardware. With your models.")]
struct Cli {
    /// Where PulseBench stores its database and caches.
    #[arg(long, global = true, env = "PULSEBENCH_DATA")]
    data_dir: Option<PathBuf>,
    /// Extra directory containing benchmark suites (one sub-directory per suite).
    #[arg(long, global = true, env = "PULSEBENCH_SUITES")]
    suites_dir: Option<PathBuf>,
    /// Print machine-readable JSON where applicable.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List providers and installed models.
    Models,
    /// Show detected hardware, runtimes and Docker.
    Hardware,
    /// List available benchmark suites.
    Suites,
    /// Run a benchmark suite against one or more models.
    Run {
        /// Suite id (for example `quick` or `full`).
        suite: String,
        /// A model id (repeatable, or comma separated). Use `provider/model` to disambiguate.
        #[arg(long = "model", short = 'm', alias = "models", value_delimiter = ',', required = true)]
        models: Vec<String>,
        /// Only run these task ids (comma separated). Makes the run non-standard.
        #[arg(long, value_delimiter = ',')]
        task: Vec<String>,
        #[arg(long)]
        temperature: Option<f32>,
        #[arg(long)]
        context: Option<u32>,
        #[arg(long)]
        max_tokens: Option<u32>,
        /// Per-generation timeout in seconds.
        #[arg(long)]
        timeout: Option<u32>,
        /// Execute generated code in Docker instead of a local isolated workspace.
        #[arg(long)]
        docker: bool,
    },
    /// List past runs.
    History {
        #[arg(long, default_value_t = 30)]
        limit: u32,
    },
    /// Show the leaderboard of a run.
    Show { run_id: String },
    /// Export a run as JSON, Markdown or an SVG share card.
    Export {
        run_id: String,
        #[arg(long, short, default_value = "json")]
        format: String,
        /// For `svg`: which model (`provider/model`); defaults to the winner.
        #[arg(long)]
        model: Option<String>,
        /// Write to a file instead of stdout.
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Install the toolchain a suite needs (one-time download).
    Prepare { suite: String },
    /// Work with suite folders.
    Suite {
        #[command(subcommand)]
        command: SuiteCommand,
    },
    /// Write the JSON Schemas of the public formats.
    Schema {
        #[arg(long, default_value = "packages/benchmark-schema")]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum SuiteCommand {
    /// Check that a suite folder is well-formed and matches its lock file.
    Verify { dir: PathBuf },
    /// Write `suite.lock.json` for the suite's current content (bump `version` first if tasks changed).
    Lock { dir: PathBuf },
    /// Run every task's untouched fixture and reference solution to prove the task is sound.
    Validate { dir: PathBuf },
    /// Import a custom suite folder into your data directory.
    Import { dir: PathBuf },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .with_target(false)
        .init();
    let cli = Cli::parse();
    let code = match run(cli).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    };
    std::process::exit(code);
}

type Res<T> = Result<T, Box<dyn std::error::Error>>;

async fn app(cli: &Cli) -> Res<Arc<App>> {
    let paths = AppPaths::detect(cli.data_dir.clone(), cli.suites_dir.clone());
    Ok(App::new(paths).await?)
}

async fn run(cli: Cli) -> Res<i32> {
    match &cli.command {
        Command::Schema { out } => {
            std::fs::create_dir_all(out)?;
            for (name, v) in [
                ("pulsebench-result-v1.schema.json", pulsebench_types::schema::result_schema()),
                ("pulsebench-suite-v1.schema.json", pulsebench_types::schema::suite_schema()),
                ("pulsebench-task-v1.schema.json", pulsebench_types::schema::task_schema()),
            ] {
                std::fs::write(out.join(name), serde_json::to_string_pretty(&v)? + "\n")?;
                println!("wrote {}", out.join(name).display());
            }
            return Ok(0);
        }
        Command::Suite { command } => return suite_command(&cli, command).await,
        _ => {}
    }

    let app = app(&cli).await?;
    match &cli.command {
        Command::Models => {
            let statuses = app.provider_statuses().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&statuses)?);
            } else {
                print!("{}", render::models(&statuses));
            }
            Ok(if statuses.iter().any(|s| s.state == ConnectionState::Connected) { 0 } else { 2 })
        }
        Command::Hardware => {
            let info = app.system_info(true).await;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&info)?);
            } else {
                print!("{}", render::hardware(&info));
            }
            Ok(0)
        }
        Command::Suites => {
            let list = app.suites()?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&list)?);
            } else {
                print!("{}", render::suites(&list));
            }
            Ok(0)
        }
        Command::Prepare { suite } => {
            let rep = app.prepare_suite(suite, &tokio_util::sync::CancellationToken::new()).await?;
            println!("{}", rep.message);
            Ok(if rep.ready { 0 } else { 1 })
        }
        Command::Run { suite, models, task, temperature, context, max_tokens, timeout, docker } => {
            let statuses = app.provider_statuses().await?;
            let selections = render::resolve_models(&statuses, models)?;
            let mut settings: RunSettings = app.settings()?.run;
            if let Some(t) = temperature {
                settings.generation.temperature = *t;
            }
            if let Some(c) = context {
                settings.generation.context_tokens = *c;
            }
            if let Some(m) = max_tokens {
                settings.generation.max_output_tokens = *m;
            }
            if let Some(t) = timeout {
                settings.generation.timeout_seconds = *t;
            }
            if *docker {
                settings.execution = pulsebench_types::ExecutionMode::Docker;
            }
            let req = StartRunRequest {
                suite_id: suite.clone(),
                models: selections,
                task_ids: if task.is_empty() { None } else { Some(task.clone()) },
                settings: Some(settings),
            };
            let mut rx = app.events();
            let launched = app.launch_run(req).await?;
            let a2 = app.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    eprintln!("\ncancelling… (finished tasks are kept)");
                    a2.cancel_run();
                }
            });
            let json = cli.json;
            let printer = tokio::spawn(async move {
                while let Ok(ev) = rx.recv().await {
                    if json {
                        continue;
                    }
                    if let Some(line) = render::event_line(&ev) {
                        println!("{line}");
                    }
                    if matches!(ev, RunEvent::RunFinished { .. }) {
                        break;
                    }
                }
            });
            let run = launched.handle.await?;
            let _ = printer.await;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&run)?);
            } else {
                println!();
                print!("{}", render::leaderboard(&run));
                println!("\nRun ID: {}   (export with `pulsebench export {} --format markdown`)", run.id, run.id);
            }
            Ok(match run.status {
                RunStatus::Completed => 0,
                RunStatus::Cancelled => 130,
                _ => 1,
            })
        }
        Command::History { limit } => {
            let runs = app.list_runs(*limit)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&runs)?);
            } else {
                print!("{}", render::history(&runs));
            }
            Ok(0)
        }
        Command::Show { run_id } => {
            let run = app.get_run(run_id)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&run)?);
            } else {
                print!("{}", render::leaderboard(&run));
            }
            Ok(0)
        }
        Command::Export { run_id, format, model, output } => {
            let fmt = match format.as_str() {
                "json" => ExportFormat::Json,
                "md" | "markdown" => ExportFormat::Markdown,
                "svg" | "card" => ExportFormat::Svg,
                other => return Err(format!("unknown format '{other}' (json, markdown, svg)").into()),
            };
            match output {
                Some(path) => {
                    app.export_run_to_file(run_id, fmt, model.as_deref(), path)?;
                    eprintln!("wrote {}", path.display());
                }
                None => print!("{}", app.export_run(run_id, fmt, model.as_deref())?),
            }
            Ok(0)
        }
        Command::Schema { .. } | Command::Suite { .. } => unreachable!(),
    }
}

async fn suite_command(cli: &Cli, cmd: &SuiteCommand) -> Res<i32> {
    use pulsebench_core::{load_suite, write_lock, LockStatus};
    match cmd {
        SuiteCommand::Verify { dir } => {
            let s = load_suite(dir)?;
            println!("{} v{}: {} tasks, hash {}", s.manifest.name, s.manifest.version, s.tasks.len(), s.content_hash);
            match s.lock {
                LockStatus::Verified => {
                    println!("lock file matches ✓");
                    Ok(0)
                }
                LockStatus::Missing => {
                    println!("no suite.lock.json (custom suites do not need one)");
                    Ok(if s.manifest.official { 1 } else { 0 })
                }
                LockStatus::Mismatch { expected, actual } => {
                    println!("MISMATCH: content changed without a version bump\n  locked: {expected}\n  actual: {actual}");
                    Ok(1)
                }
            }
        }
        SuiteCommand::Lock { dir } => {
            let lock = write_lock(dir)?;
            println!("wrote {}/suite.lock.json (v{}, {} files, {})", dir.display(), lock.version, lock.files, lock.content_hash);
            Ok(0)
        }
        SuiteCommand::Validate { dir } => {
            let suite = load_suite(dir)?;
            let app = app(cli).await?;
            let tc = match &suite.manifest.toolchain.node {
                Some(spec) => Some(
                    pulsebench_sandbox::toolchain::prepare_node_toolchain(
                        &app.paths.toolchain_dir(),
                        spec,
                        suite.node_lockfile.as_deref(),
                        &tokio_util::sync::CancellationToken::new(),
                        &|m| eprintln!("{m}"),
                    )
                    .await?,
                ),
                None => None,
            };
            let work = app.paths.work_dir().join("validate");
            let results = pulsebench_core::validate::validate_suite(&suite, &work, tc).await;
            let _ = std::fs::remove_dir_all(&work);
            let mut bad = 0;
            for r in &results {
                if r.ok() {
                    println!("✓ {}  (fixture fails, reference solution passes)", r.task_id);
                } else {
                    bad += 1;
                    println!("✗ {}", r.task_id);
                    for p in &r.problems {
                        println!("    {p}");
                    }
                }
            }
            Ok(if bad == 0 { 0 } else { 1 })
        }
        SuiteCommand::Import { dir } => {
            let app = app(cli).await?;
            let s = app.import_suite(dir)?;
            println!("imported '{}' v{} ({} tasks) into {}", s.name, s.version, s.task_count, app.paths.user_suites().display());
            println!("note: custom suites run their test commands on your machine; only import suites you trust.");
            Ok(0)
        }
    }
}
