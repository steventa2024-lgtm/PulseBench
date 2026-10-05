# Contributing to PulseBench

Thanks for helping make local-model benchmarking evidence-based. The most valuable contributions are **benchmark tasks**, **provider adapters** and **bug reports with reproducible results**.

## Ground rules

* **Nothing simulated.** No fake data, randomized scores or placeholder results in product code. If something is unavailable, show it as unavailable. Test doubles live in tests (`providers::testing`, mock servers).
* **Fairness is a feature.** Never add per-model prompt tuning or hidden settings.
* **Benchmark commands come from definitions, never from model output.**
* **Don't change a published suite silently.** Content changes require a new suite version and a refreshed lock file; CI enforces this.

## Setup

See [docs/development.md](docs/development.md). In short: `npm ci`, then `cargo test --workspace --exclude pulsebench-desktop`.

## Proposing a benchmark task

Open a *Benchmark task proposal* issue. A good task:

1. has a runnable fixture, tests that describe the requirement, a reference solution in `solution/`, and (for test-writing tasks) mutants;
2. fails untouched for a behavioural reason and passes with the solution (`pulsebench suite validate`);
3. is deterministic, offline, and finishes well inside its timeout on a laptop;
4. avoids trivia and tricks: a competent engineer should consider the requirement fair;
5. does not leak its solution in the prompt or fixtures.

Add it to a **new suite version** (or a new suite), run `pulsebench suite validate` and `pulsebench suite lock`, and note it in `CHANGELOG.md`.

## Adding a provider

Follow [docs/providers.md](docs/providers.md). Include parsing unit tests and a mock-server integration test; CI must not need the real service.

## Pull requests

* Keep PRs focused; describe what changed and how you verified it.
* Run `cargo fmt --all`, `cargo clippy --workspace --all-targets --exclude pulsebench-desktop -- -D warnings`, the Rust tests, `npm run typecheck && npm run test:ui`.
* If you touch Rust types: `npm run gen:types && npm run gen:schema` and commit the generated files.
* UI changes: attach a screenshot (`npm run e2e` writes them to `docs/screenshots`).
* Be explicit about anything you could not verify (for example "not tested on Windows").

## Code style

Rust: `rustfmt` (config in `rustfmt.toml`), `thiserror` error types, no `unwrap` on user-reachable paths, comments only where they add information. TypeScript: strict mode, no `any`, small components, generated types for anything that crosses the IPC boundary.

By contributing you agree that your contributions are licensed under the MIT license.
