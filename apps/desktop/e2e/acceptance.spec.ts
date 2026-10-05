import { test, expect } from "@playwright/test";
import { ChildProcess, spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

const repo = resolve(__dirname, "../../..");
const shots = join(repo, "docs", "screenshots");
let procs: ChildProcess[] = [];

async function waitFor(url: string, tries = 100): Promise<void> {
  for (let i = 0; i < tries; i++) {
    try {
      await fetch(url);
      return;
    } catch {
      await new Promise((r) => setTimeout(r, 150));
    }
  }
  throw new Error(`timed out waiting for ${url}`);
}

test.beforeAll(async () => {
  mkdirSync(shots, { recursive: true });
  const data = mkdtempSync(join(tmpdir(), "pulsebench-e2e-"));
  const mock = spawn("node", [join(repo, "scripts/mock-ollama.mjs")], { stdio: "inherit", env: { ...process.env, PORT: "11434" } });
  const bridge = spawn(join(repo, "target/debug/pulsebench-bridge"), ["--listen", "127.0.0.1:8787", "--data-dir", data, "--suites-dir", join(repo, "benchmarks")], { stdio: "inherit" });
  const vite = spawn("npx", ["vite", "--port", "1420", "--strictPort"], { cwd: resolve(__dirname, ".."), stdio: "ignore" });
  procs = [mock, bridge, vite];
  await Promise.all([waitFor("http://127.0.0.1:11434/api/version"), waitFor("http://127.0.0.1:8787/events".replace("/events", "/rpc/get_data_info")), waitFor("http://127.0.0.1:1420")]);
});

test.afterAll(() => {
  for (const p of procs) p.kill("SIGTERM");
});

test("install to first benchmark to exported report", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => m.type() === "error" && !m.text().includes("favicon") && errors.push(m.text()));

  // --- first run: welcome screen with real detection --------------------------------------------
  await page.goto("/?bridge=http://127.0.0.1:8787");
  await expect(page.getByText("Benchmark coding AI on your hardware.")).toBeVisible();
  await expect(page.getByText("qwen2.5-coder:7b").first()).toBeVisible({ timeout: 30_000 });
  await expect(page.getByText("codellama:13b").first()).toBeVisible();
  await expect(page.getByText(/Ollama/).first()).toBeVisible();
  await page.screenshot({ path: join(shots, "01-welcome.png") });

  // --- new benchmark with both models ------------------------------------------------------------
  await page.getByRole("button", { name: "RUN QUICK BENCHMARK" }).click();
  await expect(page.getByRole("heading", { name: "New benchmark" })).toBeVisible();
  await expect(page.getByLabel("Select qwen2.5-coder:7b")).toBeChecked();
  await page.getByLabel("Select codellama:13b").check();
  await expect(page.getByRole("button", { name: /START BENCHMARK · 2 MODELS/ })).toBeEnabled();
  await page.screenshot({ path: join(shots, "02-new-benchmark.png") });
  await page.getByRole("button", { name: /START BENCHMARK/ }).click();

  // --- live run ---------------------------------------------------------------------------------
  await expect(page.getByText("QUICK CODING")).toBeVisible();
  await expect(page.getByText(/GENERATING|RUNNING TESTS|APPLYING PATCH|COMPILING|PARSING/).first()).toBeVisible({ timeout: 90_000 });
  await page.screenshot({ path: join(shots, "03-live-run.png") });
  await expect(page.getByRole("button", { name: "View results" })).toBeVisible({ timeout: 200_000 });
  await page.screenshot({ path: join(shots, "04-run-finished.png") });
  await page.getByRole("button", { name: "View results" }).click();

  // --- leaderboard: two independently scored models ---------------------------------------------------
  await expect(page.getByRole("heading", { name: /QUICK CODING/ })).toBeVisible();
  const rows = page.locator("tbody tr");
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(0)).toContainText("qwen2.5-coder:7b");
  await expect(rows.nth(1)).toContainText("codellama:13b");
  const score = async (i: number) => Number((await rows.nth(i).locator("td").nth(3).innerText()).replace(/,/g, ""));
  expect(await score(0)).toBeGreaterThan(await score(1));
  await expect(rows.nth(0)).toContainText("100%");
  await expect(page.getByText(/outperformed codellama:13b by/)).toBeVisible();
  await page.screenshot({ path: join(shots, "05-leaderboard.png") });

  // sorting
  await page.getByRole("button", { name: /Tok\/s/ }).click();
  await expect(rows.nth(0)).toContainText("qwen2.5-coder:7b");

  // exports in the browser
  const [dl1] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "Export JSON" }).click()]);
  const json = JSON.parse(readFileSync((await dl1.path())!, "utf8"));
  expect(json.schema).toBe("pulsebench-result-v1");
  expect(json.models).toHaveLength(2);
  const [dl2] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "Export Markdown" }).click()]);
  expect(readFileSync((await dl2.path())!, "utf8")).toContain("# PulseBench Report");

  // --- model detail and task details ----------------------------------------------------------------
  await page.getByRole("button", { name: "codellama:13b" }).click();
  await expect(page.getByRole("heading", { name: "codellama:13b" })).toBeVisible();
  await expect(page.getByText("How this score was calculated")).toBeVisible();
  await expect(page.getByText("Category breakdown")).toBeVisible();
  await page.screenshot({ path: join(shots, "06-model-detail.png"), fullPage: false });
  await page.getByText("Repair the todo list component").click();
  const drawer = page.getByLabel("Task react-bug-001");
  await expect(drawer).toBeVisible();
  await drawer.getByRole("tab", { name: "Diff" }).click();
  await expect(drawer.getByText("@@").first()).toBeVisible();
  await drawer.getByRole("tab", { name: "Tests" }).click();
  await expect(drawer.getByText(/tests/i).first()).toBeVisible();
  await drawer.getByRole("tab", { name: "Raw output" }).click();
  await expect(drawer.getByText(/"changes"/).first()).toBeVisible();
  await drawer.getByRole("tab", { name: "Metrics" }).click();
  await expect(drawer.getByText("Completion tokens")).toBeVisible();
  await expect(drawer.getByText("unavailable").first()).toBeVisible(); // no GPU in CI: shown honestly
  await page.screenshot({ path: join(shots, "07-task-detail.png") });
  await drawer.getByRole("button", { name: "Close task details" }).click();

  // the recovered (markdown-wrapped) answer is visible in the recovery notes
  await page.getByText("Write unit tests for slugify").click();
  await expect(page.getByLabel("Task py-test-001").getByText(/markdown code fence/)).toBeVisible();
  await page.getByLabel("Task py-test-001").getByRole("button", { name: "Close task details" }).click();

  // share card
  await page.getByRole("button", { name: "Share card" }).click();
  await expect(page.getByRole("img", { name: "PulseBench share card" })).toBeVisible();
  await page.screenshot({ path: join(shots, "08-share-card.png") });
  await page.getByRole("button", { name: "Close" }).click();

  // --- compare -----------------------------------------------------------------------------------------
  await page.getByRole("button", { name: "Compare", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Compare models" })).toBeVisible();
  await expect(page.getByText("PulseBench Score").first()).toBeVisible();
  await page.screenshot({ path: join(shots, "09-compare.png") });

  // --- persistence: reload the whole app, the run is still there ---------------------------------------------
  await page.reload();
  await page.getByRole("button", { name: "History" }).click();
  await expect(page.getByRole("heading", { name: "History" })).toBeVisible();
  await expect(page.getByText("Quick Coding").first()).toBeVisible();
  await page.screenshot({ path: join(shots, "10-history.png") });

  // --- dashboard, hardware, settings ----------------------------------------------------------------------
  await page.getByRole("button", { name: "Dashboard" }).click();
  await expect(page.getByText("Installed models")).toBeVisible();
  await page.screenshot({ path: join(shots, "11-dashboard.png") });
  await page.getByRole("button", { name: "Hardware" }).click();
  await expect(page.getByText("Live utilization")).toBeVisible();
  await page.screenshot({ path: join(shots, "12-hardware.png") });
  await page.getByRole("button", { name: "Settings" }).click();
  await expect(page.getByText("The defaults are standardized")).toBeVisible();
  await page.screenshot({ path: join(shots, "13-settings.png") });

  expect(errors, `browser errors: ${errors.join("\n")}`).toEqual([]);
});
