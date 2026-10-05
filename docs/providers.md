# Providers

A provider implements `InferenceProvider` (`crates/providers/src/lib.rs`): `detect`, `list_models`, `model_info`, `generate` (streaming, cancellable, with a hard timeout) and `health`, plus optional `warm_up`, `unload` and `residency`. Benchmark logic never mentions a concrete provider.

## Ollama

* Detection: `GET /api/version`. Models: `/api/tags` enriched by `/api/show` (architecture, context length, capabilities — embedding-only models are hidden) and `/api/ps` (loaded state).
* Generation: streaming `POST /api/chat` with `options: { temperature, seed, num_ctx, num_predict }`, `keep_alive: 30m`. Token counts and rates come from Ollama's own `prompt_eval_count`, `eval_count`, `eval_duration`, `load_duration`. Thinking tokens (`message.thinking`) are captured separately and counted in tokens.
* Warm-up/unload: an empty `/api/chat` request with `num_ctx` (so the model is loaded once with the run's context size) and `keep_alive: 0`.

## LM Studio / OpenAI-compatible

* Detection: `GET {base}/models`. LM Studio's richer `GET {root}/api/v0/models` adds architecture, quantization, context length, loaded state and removes embedding models.
* Generation: streaming `POST {base}/chat/completions` with `temperature`, `seed`, `max_tokens`, `stream_options.include_usage`. If the server returns `usage`, tokens are exact and tokens/s is *client-measured* (after the first token); otherwise token metrics are `null`.
* Context length cannot be set through this API; the model's load-time context applies and the run notes say so.

## Adding a provider

1. Implement the trait in `crates/providers/src/<name>.rs` (parse functions free of I/O so they can be unit-tested).
2. Add a `ProviderKind` variant in `crates/types/src/config.rs` and wire `build_provider`.
3. Add mock-server integration tests in `crates/providers/tests` (see `http_mock.rs`).
4. Report only what the provider really reports; leave the rest `None`.
