# Methodology

## What is measured

For every (model, task) pair PulseBench records: the exact prompt, the raw model output, how the output was parsed, the applied changes and a unified diff, the compile and test command results (exit code, stdout/stderr, duration), test counts, mutant results (for test-writing tasks), generation metrics (prompt/completion/total tokens, wall time, time to first token, tokens/second and where it came from, model load time, finish reason) and resource samples (CPU, RAM, and GPU utilization/VRAM/temperature when `nvidia-smi` is available).

Anything that could not be measured is `null` in the result and *n/a* / *unavailable* in the UI. Nothing is estimated or simulated.

## Fairness rules

1. Identical system prompt (`pulsebench-prompt-v1`) and user prompt for every model on attempt 1. No per-model prompt tuning.
2. Identical files: only the task's entry files plus its read-only context files are shown.
3. Identical limits: temperature, seed, context size, max output tokens, generation timeout, attempts, test timeout.
4. Identical verification: same compile/test commands, same mutants.
5. Models run one at a time, tasks one at a time (serial), so timing and VRAM readings are not contaminated by concurrent work.
6. A model is warmed up (loaded with the run's context size) before its first task so load time does not pollute the first task's latency, and unloaded afterwards so the next model's VRAM reading starts clean.
7. Each model gets a clean workspace per task; the fixture on disk is never modified (tests assert this).

**Determinism.** PulseBench requests greedy decoding (temperature 0, seed 42) and says so in the result. It does **not** claim reproducibility to the bit: GPU kernels, drivers, quantization kernels and provider versions can change outputs. Provider versions and capabilities are recorded with each run. Providers that cannot honour a setting (e.g. OpenAI-compatible servers and context length) are called out in the run's notes.

## Output protocol

Models must answer with one JSON object:

```json
{"changes":[{"path":"src/users.ts","content":"<complete file>"}],"explanation":"…"}
```

Recovery (each recorded in `parse.notes`, mode `recovered`): `<think>` blocks removed, JSON in markdown fences, JSON surrounded by prose, unescaped newlines/tabs and trailing commas, alternative key names (`file`/`code`), and — only when unambiguous — plain code blocks mapped to the single editable file or identified by a path hint. Truncation (a JSON object that never closes, or `finish_reason: length`) is diagnosed and counted as a reliability failure.

Changes are validated: normalized relative paths only, no `..`, no absolute paths or drive letters, only the task's editable files, no empty files, a 1 MB cap, no writes through symlinks. Rejected changes are recorded with the reason.

## Attempts and repair

`maxAttempts` per task (1 in Quick, 2 in most Full tasks). On a failed attempt the next prompt contains the previous attempt's *real* compile/test output (truncated to 4,000 characters) and the current file contents.

## Verification

1. Compile / type-check (if the task defines one). A compile failure ends the attempt.
2. Test command. Success requires exit code 0 **and** at least one test (an empty run is `no-tests-found`).
3. Mutation (test-writing tasks): the model's tests must pass on the correct implementation, contain at least `minTests` tests, and fail on every mutant. For compiled languages each mutant is rebuilt first. A mutant that does not compile is a benchmark defect, never a "kill" (the suite validator catches it).

Test counts are parsed from `node --test` (TAP and spec reporters) and Python `unittest`; if counts cannot be parsed the exit code remains authoritative.

## Task statuses

| Status | Meaning | Scored |
|---|---|---|
| passed | all verification passed | yes |
| failed | the model's answer failed (including timeouts and unparseable output) | yes (as a failure) |
| error | infrastructure/provider error (provider down, workspace write failure) | yes (as a failure, flagged) |
| skipped | the host cannot run the task (missing runtime, Docker not running) | no — excluded and reported |

A model that errors on 3 consecutive tasks is abandoned and marked failed. A model that cannot be loaded (e.g. insufficient memory) is `failed` with the provider's message and gets no score (DNF), not a zero.

## Suite integrity

Suites are versioned. `suite.lock.json` stores the SHA-256 content hash (line endings normalized) for the declared version. Loading a suite whose content differs from its lock marks it *modified* — the run is not labelled official. CI fails if official content changes without a version bump. Every task is validated by running its untouched fixture (must fail) and its reference solution (must pass).
