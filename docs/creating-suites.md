# Creating benchmark suites

```text
my-suite/
  suite.json
  suite.lock.json            # optional; required for suites that declare "official": true
  toolchain/package-lock.json  # optional; pins the Node toolchain used by TypeScript/React tasks
  tasks/
    task-001/
      task.json
      workspace/             # the runnable project the model sees (fixture)
      solution/              # reference solution: the same relative paths as the editable files
      mutants/…              # for "write tests" tasks: broken implementations
```

## `suite.json`

```json
{
  "schemaVersion": 1,
  "id": "my-suite",
  "name": "My Suite",
  "version": "1.0.0",
  "description": "What it measures.",
  "official": false,
  "estimatedMinutes": "10-20",
  "tasks": ["task-001"],
  "toolchain": { "node": { "packages": { "typescript": "5.9.3", "@types/node": "22.20.5" } } }
}
```

## `task.json`

```json
{
  "id": "task-001",
  "title": "Repair user filtering",
  "category": "bugfix",
  "language": "typescript",
  "difficulty": "easy",
  "description": "Repair the implementation so all included tests pass.",
  "entryFiles": ["src/users.ts"],
  "editableFiles": ["src/users.ts"],
  "contextFiles": ["test/users.test.ts"],
  "compileCommand": "node node_modules/typescript/bin/tsc -p tsconfig.json",
  "testCommand": "node --test dist/test/*.test.js",
  "timeoutSeconds": 120,
  "maxAttempts": 1
}
```

* `category`: `bugfix | implementation | refactoring | testing | security | reasoning`
* `language`: `python | javascript | typescript | react`
* `entryFiles` are shown to the model; `editableFiles` (default = entry files) are the only paths it may write — they may not exist yet (test-writing tasks); `contextFiles` are shown read-only.
* Commands are **never** model-supplied. They are run without a shell and must start with an allowed program (`python`, `node`, `npm`). Inline-code flags (`python -c`, `node -e`) are rejected. Use `node node_modules/typescript/bin/tsc` and `node --test <glob>` rather than shims so commands work on every OS.
* Python tasks use `python -m unittest discover`; JS/TS/React tasks use `node --test` (React tests use `jsdom`; see the official suites for a helper).
* Scoring weights are **not** per task: every task is scored by the same formula so models are comparable.

### "Write tests" tasks

```json
"verification": {
  "kind": "mutation",
  "minTests": 6,
  "mutants": [
    { "id": "m1", "description": "does not lowercase", "overlay": { "slugify.py": "mutants/m1/slugify.py" } }
  ]
}
```

The model's tests must pass on the fixture's (correct) implementation and fail on every mutant. For compiled languages each mutant is recompiled.

## Validate before you publish

```bash
pulsebench suite validate my-suite     # untouched fixture must FAIL, reference solution must PASS (mutants all killed)
pulsebench suite verify my-suite       # structure + lock status
pulsebench suite import my-suite       # copy into your data directory (or use Suites → Import)
```

Good tasks have: a failing baseline for a *behavioural* reason, tests that describe the requirement (hide edge cases from the prompt by leaving them out of `contextFiles` if you want to test generalization), no network access, deterministic results and a runtime under the timeout on a slow machine.

## Versioning rules

Changing a task, fixture, test, mutant or toolchain lock **requires a new suite `version`** and a refreshed lock (`pulsebench suite lock my-suite`). Official suites in this repository are checked by CI; PulseBench labels a suite *official* only if its content matches its lock file.

## Trust

Importing a suite means running its test commands on your machine (inside the sandbox described in [security-model.md](security-model.md), which limits but does not eliminate risk in local mode). Import only suites you trust, or use Docker execution.
