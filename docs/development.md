# Development

## Prerequisites

* Rust stable (≥ 1.80), Node.js 20+ (22 recommended), Python 3.8+ (to run Python tasks and tests).
* **Windows:** [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on Windows 11) and the MSVC build tools.
* **Linux:** `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev pkg-config build-essential`.
* **macOS:** Xcode command line tools. (macOS/Linux builds are not part of the MVP.)

## Common tasks

```bash
npm ci                                  # JS dependencies (workspaces)
cargo test --workspace --exclude pulsebench-desktop        # Rust tests (needs python3 + node; first run downloads the pinned toolchain)
npm run typecheck && npm run test:ui    # UI type check and unit tests
npm run gen:types                       # regenerate TypeScript types (ts-rs) → packages/types
npm run gen:schema                      # regenerate JSON Schemas → packages/benchmark-schema
npm run desktop:dev                  # Tauri dev (hot reload)
npm run desktop:build                # installer (NSIS/MSI on Windows)
cargo run -p pulsebench-cli -- --help
```

### UI without the desktop shell

```bash
cargo run -p pulsebench-bridge          # real backend over loopback HTTP (dev/test only)
npm run dev                             # http://127.0.0.1:1420/?bridge=http://127.0.0.1:8787
npm run e2e             # Playwright: starts a mock Ollama, the bridge and Vite, drives the whole UI
```

Set `PLAYWRIGHT_CHROMIUM` to a Chromium executable if Playwright's own browser is not installed.

### Checks that CI runs

`cargo fmt --check`, `cargo clippy -D warnings`, all Rust tests (including the validation of every official task), `tsc --noEmit`, Vitest, a Vite build, a generated-files drift check, and the Playwright flow.

## Changing benchmark content

Edit tasks → bump the suite `version` → `pulsebench suite validate benchmarks/<suite>` → `pulsebench suite lock benchmarks/<suite>` → update `CHANGELOG.md`.

## Data locations

`%APPDATA%\ZeroPulse\PulseBench` on Windows (`~/.local/share/ZeroPulse/PulseBench` on Linux, `~/Library/Application Support/ZeroPulse/PulseBench` on macOS). Override with `PULSEBENCH_DATA`. Bundled suites are found next to the executable or via `PULSEBENCH_SUITES`.
