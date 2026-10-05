# Scoring

All scores are computed from stored task results; the formula is versioned (`pulsebench-score-v1`) and recorded with every run.

## Task components (each `0..1`)

| Component | Definition |
|---|---|
| `correctness` | `1` if verification fully passed, else `0` |
| `tests` | Final attempt's test pass fraction (`passed/total`). For test-writing tasks: fraction of mutants killed, `0` if the tests fail on the correct code or are too few |
| `compile` | `1` if the compile step succeeded; *absent* if the task has no compile step |
| `efficiency` | solved on attempt `k` of `maxAttempts`: `1 − (k−1)/maxAttempts`; unsolved: `0` |
| `speed` | mean tokens/s of the task's attempts mapped on a log scale: `ln(tps/floor) / ln(ceiling/floor)` clamped to `[0,1]` (defaults 5 → 0, 100 → 1); *absent* if no token rate was reported |
| `reliability` | per attempt: clean JSON `1`, recovered `0.5`, failed / truncated / timed out / provider error `0`; averaged over attempts |

Speed is intentionally hardware-dependent: PulseBench answers "which model is best *on my hardware*".

## Aggregation

For a set of tasks (one task, a category, a whole model) each component is the **difficulty-weighted mean** over tasks where it applies (default weights easy 1, medium 2, hard 3). Then

```
score = Σ wᵢ · cᵢ / Σ wᵢ      over components that applied
PulseBench Score = round(10 000 × score)
```

Default weights: correctness 60, tests 20, compile 5, efficiency 5, speed 5, reliability 5. Weights of components that did not apply are dropped and the rest renormalized, so a model is never penalized for a metric the provider cannot report. Category scores (0–100) use the same formula over the category's tasks. The model detail view shows every component, its weight and its points.

## Worked example

A single *easy* task, failed: 3/4 tests passed, compiled, recovered output, 20 tok/s.

```
correctness 0, tests 0.75, compile 1, efficiency 0, speed ln(20/5)/ln(100/5)=0.463, reliability 0.5
score = (60·0 + 20·0.75 + 5·1 + 5·0 + 5·0.463 + 5·0.5) / 100 = 0.2481   →  2,481
```

(This exact example is a unit test in `crates/scoring`.)

## Comparing runs

Scores are comparable when the suite id **and version** match and both runs used standard settings. The History view marks changes between runs that differ in either (`⚠`) so a new suite version or a changed temperature is never mistaken for a regression.
