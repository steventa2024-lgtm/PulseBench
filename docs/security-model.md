# Security model

## Threat model

Model output is **untrusted code**. Benchmark definitions are **trusted** (they are code too — treat third-party suites accordingly).

## What PulseBench guarantees

* **The model never chooses commands.** Compile and test commands come only from the task definition. Model output can only supply file contents for the task's editable files.
* **Path safety.** Writes use normalized relative paths; absolute paths, drive letters, `..`, NUL bytes and paths through symlinks are rejected (backslashes are normalized to separators first); only the task's editable files are accepted; reserved locations (`node_modules`, `.pb-*`) cannot be written. Fixtures containing symlinks are not copied.
* **Command policy.** Commands are split without a shell and must start with `python`, `node` or `npm`; inline-code flags are rejected; programs cannot be given as paths.
* **Environment sanitization.** Benchmark processes start with an *empty* environment plus `PATH`, `HOME`/`USERPROFILE`/temp redirected into the workspace, `CI=1`, `NO_COLOR=1`, `NODE_ENV=test`, deterministic Python flags. Your tokens, cloud credentials and API keys are not inherited. (The trusted one-time toolchain install additionally forwards proxy and CA variables.)
* **Isolation per task.** A new disposable directory per (model, task); the fixture on disk is never modified; workspaces are deleted afterwards (unless *Keep workspaces* is enabled).
* **Resource control.** Timeouts per command, output capture capped (2 MB buffered, 96 KB kept per stream), the whole process tree is killed on timeout/cancel (process group on Unix, `taskkill /T` on Windows), no orphan processes after cancellation.
* **Privacy.** No telemetry, no uploads, no accounts. Provider API keys (custom endpoints only) are stored in the local database, masked in the UI and never written to exports or results.
* **The shared Node toolchain is read-only** (files and directories lose their write bit; mounted `:ro` in Docker) so one model cannot poison the next model's run.

## What local mode does **not** guarantee

Local mode is a *clean working directory with a sanitized environment*, not a sandbox in the operating-system sense. Code executed by a test can still read any file your user account can read, make network connections and consume CPU/RAM within its timeout. If you benchmark models whose output you do not want to trust at that level, use **Docker mode**: `docker run --rm --network none --memory 4g --cpus 2 --pids-limit 512 --cap-drop ALL --security-opt no-new-privileges`, only the workspace mounted (and the toolchain read-only), run as your uid on Unix. The UI and every result state which mode was used and what it does and does not isolate.

## Reporting vulnerabilities

See [SECURITY.md](../SECURITY.md).
