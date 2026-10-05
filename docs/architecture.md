# Architecture

PulseBench is a Rust workspace plus a React UI. **All benchmark behaviour lives in Rust crates that the desktop app and the CLI share**; the UI is a typed client.

| Crate / package | Responsibility |
|---|---|
| `crates/types` | The data model: suite/task format, run configuration, the public `pulsebench-result-v1` format, events. Generates TypeScript (`ts-rs`) and JSON Schema (`schemars`). |
| `crates/providers` | `InferenceProvider` trait (`detect`, `list_models`, `model_info`, `generate`, `health`, `warm_up`, `unload`, `residency`), Ollama and OpenAI-compatible implementations, error mapping, a scripted provider for tests. |
| `crates/telemetry` | Hardware detection (sysinfo, `nvidia-smi`, OS tools for non-NVIDIA names) and the 1 Hz resource sampler. |
| `crates/sandbox` | Disposable workspaces, safe file writes, command allowlist, sanitized environments, process execution with timeouts / output caps / process-tree kill, Docker execution, toolchain cache. |
| `crates/scoring` | Pure functions: task components, aggregation, categories, statistics. |
| `crates/storage` | SQLite with embedded migrations; settings, providers, runs, task results, telemetry. |
| `crates/benchmark-core` | Suite loading/validation/locking, prompt builder, response protocol parser, verification (compile → tests → mutants), the runner, Markdown/SVG reports. |
| `crates/app` | The service used by every front end: settings, discovery, starting/cancelling runs, history, exports, and the JSON command surface (`rpc.rs`). |
| `crates/cli` | The `pulsebench` binary. |
| `crates/bridge` | Dev/test-only HTTP bridge to the same command surface (loopback only). |
| `apps/desktop` | React 19 UI and the Tauri 2 shell in `src-tauri` (one `rpc` command + one event channel). |
| `packages/types`, `packages/benchmark-schema` | Generated TypeScript types and JSON Schemas. |
| `benchmarks/` | Official suites (`quick`, `full`) with fixtures, reference solutions, mutants and lock files. |

## Run flow

```text
App::start_run
  resolve models against live providers (model_info)  → RunRequest
  run_benchmark:
    prepare toolchain (Node, once, pinned lockfile) / detect runtimes  → unavailable runtimes ⇒ tasks Skipped
    per model:   warm_up (load with the run's context size) → residency → tasks → unload
    per task:    fresh workspace copy → baseline (cached per run) → prompt
                 attempt loop: generate (stream, cancel, timeout) → parse → validate paths → apply
                               → compile → tests → mutants → resources → score
                 destroy workspace
    after each task: checkpoint to SQLite (crash-safe history); events stream to UI/CLI
  finalize: save normalized tables + the authoritative result JSON
```

Progress is streamed as `RunEvent`s over a broadcast channel; the desktop shell forwards them to the web view.
The UI never blocks on benchmark work: it lives in tokio tasks and child processes.

## Typed IPC

`crates/app/src/rpc.rs` defines the command surface. `apps/desktop/src/ipc/commands.ts` wraps every command in a typed function using generated types, so no untyped payload reaches UI code. A Rust test asserts that every advertised command is routed, and a UI test asserts that every command has a wrapper.

## Persistence

`benchmark_runs.result_json` is authoritative; `benchmark_models`, `task_results`, `benchmark_tasks` and `telemetry_samples` are denormalized for listing, comparison and history queries. Migrations are append-only (`crates/storage/migrations`) and tracked in `schema_migrations`; a test upgrades a v1 database in place and checks that history survives.

## Extension points

* **Providers** — implement `InferenceProvider` and add it to `build_provider`.
* **Suites** — add a folder; see [creating-suites.md](creating-suites.md).
* **Verification kinds** — `Verification` in `crates/types/src/suite.rs` (currently `tests` and `mutation`).
* **Languages/runtimes** — `Language`, `Runtime`, the command allowlist in `sandbox::policy` and `Sandbox::docker_request`.
