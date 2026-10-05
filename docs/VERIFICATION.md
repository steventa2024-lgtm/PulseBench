# Verification report

This report separates **what was actually executed** during development from **what could not be verified** in the build environment
(a Linux container with no GPU, no Ollama, no LM Studio, no Docker daemon and no Windows). Nothing below is claimed beyond what was run.

## Executed and passing

| Area | How it was verified | Result |
|---|---|---|
| Rust workspace | `cargo test --workspace --exclude pulsebench-desktop` | **196 passed, 0 failed** (from a cold toolchain cache) (includes ts-rs binding export tests) |
| Lints / format | `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` | clean |
| UI | `tsc --noEmit` (strict, `noUncheckedIndexedAccess`), Vitest, `vite build` | 40 tests pass, build OK |
| 25 benchmark tasks | `crates/benchmark-core/tests/official_suites.rs` runs, for **every task**, the untouched fixture (must fail) and the reference solution (must pass; mutation tasks must kill every mutant) through the real sandbox, real Python 3.11 / Node 22, a real `npm ci` of the pinned toolchain | all 25 valid. The validator caught two defects while authoring (an empty-baseline false pass and a mutant that was not killable) — both fixed |
| Suite lock files | same test file | both official suites match `suite.lock.json` |
| Engine end-to-end | `crates/benchmark-core/tests/engine.rs`: real runner, real workspaces and test execution, scripted providers | identical prompts for all models; independent scores (clean JSON > recovered > garbage); path-escape attempts rejected; repair attempts carry real test output; provider errors / timeouts recorded; cancellation prompt with no leftover workspaces; missing runtime ⇒ skipped, no score; result validates against the published JSON Schema |
| Providers | unit tests on response parsing + `crates/providers/tests/http_mock.rs` (in-process HTTP servers speaking the Ollama and OpenAI-compatible protocols) | discovery, enrichment, streaming, metrics, typed errors, timeout, cancel, unreachable |
| Sandbox | `crates/sandbox` tests with real processes | env not inherited, timeout kills the whole process tree (grandchild verified dead), cancel, output cap, path/symlink escapes refused, allowlist, inline-code flags rejected |
| Storage | `crates/storage` tests | migrations, in-place upgrade from a v1 database keeps history, checkpoint/finalize, interrupted-run recovery, cascade delete, DB export |
| CLI acceptance | `crates/cli/tests/e2e.rs` runs the real `pulsebench` binary against a mock Ollama server: discovery → run Quick on two models → ranked leaderboard → history in a **new process** → JSON / Markdown / SVG export → unknown-model error | pass |
| Desktop UI acceptance | `apps/desktop/e2e/acceptance.spec.ts` (Playwright + Chromium) drives the full UI against the **real backend** (`pulsebench-bridge`) and a mock Ollama: welcome → select two models → live run → leaderboard (2 independently scored rows, sorting) → JSON/Markdown download → model detail → task drawer (diff, tests, raw output, metrics, recovery notes) → share card → compare → reload and reopen history → dashboard/hardware/settings. Fails on any browser console error | pass; screenshots in `docs/screenshots` |
| Tauri shell | `tauri build --debug --no-bundle` compiled the real desktop binary; launched under Xvfb (WebKitGTK) with a mock Ollama: the native IPC path loaded settings, detected hardware and providers and rendered the welcome screen with discovered models | works (screenshot reviewed during development) |
| Packaging | `tauri build --debug --bundles deb` produced a 76 MB installer; extracting it shows `benchmarks/{quick,full}` bundled at the resource path the app searches, and `pulsebench suite verify` confirms both bundled suites still match their lock files | works on Linux (`.deb`); the Windows NSIS/MSI bundle was not built |
| CI on GitHub (PR #1, head `60ed3c1`) | all six jobs green: Rust on **ubuntu-latest and windows-latest** (fmt, clippy, full test suite), generated-files drift check, UI type check/tests/build, Playwright UI end-to-end with the real backend, desktop shell compiles | pass. CI also found two real problems that local runs had hidden: a race between concurrent Node-toolchain installs (fixed with an atomic install lock) and a Windows path-separator assumption in a test |
| Hardware detection | `pulsebench hardware` on the build container | real CPU/RAM/OS/runtimes; GPU correctly reported as *none detected*; Docker correctly reported as *installed, daemon not reachable* |
| Provider failure UX | `pulsebench models` with nothing listening | "not detected" with the real connection error, exit code 2 |

## Not verified here (needs your machine)

These are implemented, but could not be exercised in the build environment. **Run the checklist below before announcing the project.**

1. **Real Ollama and LM Studio.** The provider code is tested against mock servers whose payloads follow the documented API shapes (`/api/tags`, `/api/show`, `/api/ps`, `/api/chat` streaming with `eval_count`/`eval_duration`; LM Studio `/api/v0/models` and OpenAI-style SSE with `usage`). They were not captured from live servers, so field-level differences between versions are possible. If something differs, `pulsebench models` / the run notes will show what the provider returned.
2. **NVIDIA telemetry.** `nvidia-smi` CSV parsing is unit-tested on sample lines; live sampling during a run has never run against a real GPU. AMD/Intel GPUs are named only (no live metrics) by design.
3. **Windows desktop app and installer.** The Rust test suite (including real Python/Node execution, `npm.cmd` resolution, the `node_modules` junction, process cleanup, the CLI end-to-end flow and validation of all 25 tasks) **passes on GitHub's `windows-latest` runner**, but nobody has launched the Windows desktop app (WebView2 window) or built and installed the NSIS/MSI package; `release.yml` has not run. GPU-name detection through PowerShell and `taskkill` tree-kill on a *timed-out* command were not specifically exercised on Windows.
4. **Docker execution.** The `docker run --network none …` path is implemented and unit-structured, but there is no daemon here. With Docker unavailable PulseBench *skips* tasks and says so (tested); it does not pretend.
5. **Large models, long runs, thinking models.** Behaviour with 30B models, multi-hour Full runs and models that exhaust the token limit while "thinking" has not been exercised (truncation handling is unit-tested).
6. **Suite duration estimates** (`5–15 min`, `30–90 min`) are estimates, not measurements.
7. **Pause** is not implemented (cancel is). **Parallelism** is fixed at 1 by design (fair timing).

## Acceptance checklist for a real machine

Run this once on your RTX machine; every step maps to the final acceptance test in the project brief.

- [ ] `pulsebench hardware` shows your GPU name, VRAM, driver, `telemetry live`.
- [ ] `pulsebench models` shows Ollama connected with your real models (quantization, size, context).
- [ ] Launch the app → Welcome shows the same data → *Run quick benchmark*.
- [ ] Select two models → Start. During the run GPU %, VRAM and tokens stream live; Task Manager/`nvidia-smi` agree.
- [ ] Both models receive the same tasks (open a task for each model → *Prompt* tab is byte-identical).
- [ ] Workspaces appear and disappear under `%APPDATA%\ZeroPulse\PulseBench\work` only; nothing is written to your repositories.
- [ ] Results show different, independently computed scores; click a model → every task, diff, test output and metric is present.
- [ ] Cancel mid-run: finished tasks are kept, run marked cancelled, no stray `python`/`node` processes.
- [ ] Close and reopen the app → the run is in History → export JSON and Markdown.
- [ ] (Optional) Enable Docker mode and repeat with a small model.

If any step fails, please open a bug report with the exported JSON (it contains the evidence).

## Known limitations and honest notes

* Local execution mode is not an OS-level sandbox (see [security-model.md](security-model.md)).
* The reference solutions ship in the repository (needed to validate tasks). A model trained on this repository could memorize them; treat official-suite scores as a measure of *your* local setup, and prefer private custom suites for contamination-sensitive comparisons.
* Task difficulty and category weights are judgement calls and are documented in [scoring.md](scoring.md); the weights are configurable.
* The task format differs slightly from the original sketch: per-task `scoring` weights were replaced by one suite-wide formula so every task is scored identically, and `fixtures/<id>` became `tasks/<id>/workspace`.
* "Tokens streamed" in the live view counts streamed chunks; the final tokens/second shown in results is provider-reported (or clearly labelled client-measured).

## Phase checklist (from the project brief)

| Phase | Status |
|---|---|
| 1 Foundation (monorepo, Tauri, React shell, SQLite, provider interface, settings) | done |
| 2 System detection (hardware, NVIDIA telemetry, Ollama / LM Studio detection, model discovery) | done — live NVIDIA/Ollama/LM Studio unverified, see above |
| 3 Benchmark core (suite schema, loader, workspaces, prompts, generation pipeline, parser) | done |
| 4 Execution (apply, compile, tests, timeouts, sandbox safety) | done — Docker mode unverified |
| 5 Scoring (task, category, model, PulseBench Score) | done |
| 6 Benchmark UI (selection, live execution, progress, logs) | done |
| 7 Results (leaderboard, model detail, task detail, comparison) | done |
| 8 Persistence (history, reopen, over-time comparison) | done |
| 9 Export (JSON, Markdown, share card as SVG/PNG) | done |
| 10 Repository polish (README, docs, screenshots, CI, tests, release workflow) | done — `ci.yml` is green on GitHub; `release.yml` has not been run |
