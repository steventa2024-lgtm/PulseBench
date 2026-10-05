# `pulsebench-result-v1`

Every run is stored and exported as one JSON document whose schema is published in
[`packages/benchmark-schema/pulsebench-result-v1.schema.json`](../packages/benchmark-schema/pulsebench-result-v1.schema.json)
(generated from the Rust types; CI fails if it drifts). Tests validate real run results against it.

Top-level fields: `schema`, `id`, `createdAt`, `finishedAt`, `status`, `appVersion`, `suite` (`id`, `version`, `contentHash`, `official`),
`settings` (generation + execution + scoring configuration), `standardSettings`, `system` (OS, CPU, RAM, GPUs, runtimes, Docker),
`providers` (name, URL, version, capabilities — never credentials), `models[]`, `log[]`, `telemetry[]`, `notes[]`.

Each `models[]` entry has the model's metadata, `score` (PulseBench Score, components, per-category scores), `stats`, and `tasks[]`;
each task has `attempts[]` with the prompt, generation metrics, raw output, parse report, changes, diff, compile/test outcomes, mutants and resource summary.

To verify someone's result you need: PulseBench version, suite id + version + `contentHash`, the model and provider (with version), the hardware, the generation settings and the per-task results — all present.
Future versions will add signatures (see the roadmap).

Generate the schemas locally with `npm run gen:schema` (`pulsebench schema`).
