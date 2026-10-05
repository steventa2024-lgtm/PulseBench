# Roadmap

Architecture hooks already exist for most of these; none are claimed as implemented.

**Platforms** — macOS, Linux packaging; AMD/Intel GPU telemetry (ROCm / Level Zero) beyond naming the device.
**Providers** — llama.cpp server, vLLM, OpenRouter and other hosted OpenAI-/Anthropic-compatible APIs.
**Benchmarks** — SWE-bench and HumanEval/MBPP adapters, repository-specific benchmarks, coding-agent and multi-agent benchmarks, community suite packs, model regression testing, quantization comparison.
**Sharing** — PDF reports, hosted public reports, anonymous result uploads with signature verification, a public leaderboard, GPU-to-GPU comparison, optimal-model recommendation.
**Runtime** — pause/resume of a run (cancel is implemented; pause is not), optional parallel *execution* of tests (generation stays serial for fair timing), OS-level sandboxing for local mode.

Contributions welcome — open a "Provider request" or "Feature request" issue first.
