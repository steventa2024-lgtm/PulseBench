#!/usr/bin/env node
// A stand-in for an Ollama server, used ONLY by the UI end-to-end test (apps/desktop/e2e).
// It speaks Ollama's real HTTP API; PulseBench's provider, engine, sandbox and test execution are
// the real ones. "oracle" models answer with each task's reference solution; "sloppy" answers some
// tasks correctly, one in markdown (to exercise output recovery) and fails the rest.
import http from "node:http";
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "benchmarks", "quick", "tasks");
const port = Number(process.env.PORT ?? 11434);

function solutionFor(title) {
  for (const id of readdirSync(root)) {
    const tj = JSON.parse(readFileSync(join(root, id, "task.json"), "utf8"));
    if (tj.title !== title) continue;
    const editable = tj.editableFiles ?? tj.entryFiles;
    const changes = editable
      .filter((p) => existsSync(join(root, id, "solution", p)))
      .map((p) => ({ path: p, content: readFileSync(join(root, id, "solution", p), "utf8") }));
    return { id, changes };
  }
  return null;
}

const MODELS = [
  { name: "qwen2.5-coder:7b", size: 4683087332, details: { format: "gguf", family: "qwen2", parameter_size: "7.6B", quantization_level: "Q4_K_M" } },
  { name: "codellama:13b", size: 7365960935, details: { format: "gguf", family: "llama", parameter_size: "13B", quantization_level: "Q4_0" } },
];

function answer(model, user) {
  const title = user.split("\n")[0].replace("# Task: ", "");
  const sol = solutionFor(title);
  if (!sol) return "{}";
  const json = JSON.stringify({ changes: sol.changes, explanation: "Reference answer." });
  if (model.startsWith("qwen")) return json;
  if (["py-bug-001", "js-impl-001", "ts-type-001"].includes(sol.id)) return json;
  if (sol.id === "py-test-001") return "Here is the solution:\n```json\n" + json + "\n```\nLet me know if you need anything else!";
  const first = sol.changes[0];
  return JSON.stringify({ changes: [{ path: first.path, content: first.content.split("\n").slice(0, 3).join("\n") + "\n" }] });
}

const send = (res, obj, status = 200) => {
  res.writeHead(status, { "content-type": "application/json" });
  res.end(JSON.stringify(obj));
};

const server = http.createServer((req, res) => {
  const url = new URL(req.url, "http://x");
  let body = "";
  req.on("data", (c) => (body += c));
  req.on("end", async () => {
    if (url.pathname === "/api/version") return send(res, { version: "0.12.3-mock" });
    if (url.pathname === "/api/tags") return send(res, { models: MODELS.map((m) => ({ ...m, model: m.name, modified_at: "2026-09-01T10:00:00Z" })) });
    if (url.pathname === "/api/show") return send(res, { capabilities: ["completion"], model_info: { "general.architecture": JSON.parse(body).model.startsWith("qwen") ? "qwen2" : "llama", "qwen2.context_length": 32768, "llama.context_length": 16384 } });
    if (url.pathname === "/api/ps") return send(res, { models: [] });
    if (url.pathname === "/api/chat") {
      const b = JSON.parse(body);
      res.writeHead(200, { "content-type": "application/x-ndjson" });
      if (!b.messages?.length) return res.end(JSON.stringify({ model: b.model, done: true, done_reason: "load" }) + "\n");
      const text = answer(b.model, b.messages.at(-1).content);
      const chunks = text.match(/[\s\S]{1,24}/g) ?? [""];
      const tokens = Math.ceil(text.length / 4);
      for (const c of chunks) {
        res.write(JSON.stringify({ message: { role: "assistant", content: c }, done: false }) + "\n");
        await new Promise((r) => setTimeout(r, 25));
      }
      const tps = b.model.startsWith("qwen") ? 62.4 : 38.1;
      res.end(JSON.stringify({ message: { role: "assistant", content: "" }, done: true, done_reason: "stop", prompt_eval_count: 700, eval_count: tokens, eval_duration: Math.round((tokens / tps) * 1e9), load_duration: 800000000 }) + "\n");
      return;
    }
    send(res, { error: "not found" }, 404);
  });
});
server.listen(port, "127.0.0.1", () => console.log(`mock ollama on :${port}`));
