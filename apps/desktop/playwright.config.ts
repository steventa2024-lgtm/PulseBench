import { defineConfig } from "@playwright/test";

// The UI end-to-end test drives the REAL backend (pulsebench-bridge) with a mock Ollama HTTP server.
export default defineConfig({
  testDir: "./e2e",
  timeout: 240_000,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: "http://127.0.0.1:1420",
    viewport: { width: 1360, height: 860 },
    launchOptions: { executablePath: process.env.PLAYWRIGHT_CHROMIUM ?? "/opt/pw-browsers/chromium", args: ["--no-sandbox"] },
    acceptDownloads: true,
  },
});
