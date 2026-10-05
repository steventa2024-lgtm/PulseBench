# Changelog

All notable changes are documented here. Benchmark **suite** versions are tracked separately because they define comparability of results.

## [0.1.0] — Unreleased (MVP)

### Application
* Tauri 2 desktop app (Windows target) with React 19 UI: dashboard, new benchmark, live run, leaderboard, model and task detail, comparison, history over time, hardware, suites (with custom import), settings, first-run welcome, share card.
* `pulsebench` CLI (`models`, `hardware`, `suites`, `run`, `history`, `show`, `export`, `prepare`, `suite validate|verify|lock|import`, `schema`) sharing one engine with the desktop app.
* Providers: Ollama and OpenAI-compatible (LM Studio) with live discovery and streaming generation.
* Real hardware detection and telemetry (`sysinfo`, `nvidia-smi`); missing metrics are reported as unavailable.
* Sandboxed execution in disposable workspaces (sanitized environment, command allowlist, timeouts, process-tree kill), optional Docker execution.
* Transparent scoring (`pulsebench-score-v1`), versioned `pulsebench-result-v1` format with JSON Schema, JSON/Markdown/SVG/PNG exports.
* SQLite history with migrations, crash-safe checkpoints, over-time comparison.

### Benchmark suites
* **Quick Coding 1.0.0** — 5 tasks.
* **Full Coding 1.0.0** — 20 tasks (bug fixing, implementation, refactoring, testing, security, reasoning).
