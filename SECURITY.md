# Security policy

## Reporting a vulnerability

Please **do not** open a public issue for security problems. Use GitHub's [private vulnerability reporting](https://github.com/steventa2024-lgtm/PulseBench/security/advisories/new) on this repository with a description, reproduction steps and the affected version. We aim to acknowledge reports within 3 working days and to publish a fix or mitigation within 30 days.

## Scope

In scope: sandbox escapes in Docker mode, ways for model output to cause execution of anything other than the benchmark's own commands, path traversal in file application, leakage of environment secrets or provider credentials into benchmark processes, exports or logs, vulnerabilities in the Tauri shell/IPC, and supply-chain issues in released artifacts.

Known limitation (documented, not a vulnerability): **local execution mode is not an OS-level security boundary** — see [docs/security-model.md](docs/security-model.md). Use Docker mode for stronger isolation.

## Design principles

No telemetry, no network access beyond the providers you configure (and the one-time pinned toolchain install), no execution of model-supplied commands, least-privilege Tauri capabilities, sanitized environments for all benchmark processes.
