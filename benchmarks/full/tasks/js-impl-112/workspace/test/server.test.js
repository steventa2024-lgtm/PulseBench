"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { createServer } = require("../src/server");
const { createStore } = require("../src/store");

async function withServer(fn) {
  const store = createStore();
  const server = createServer(store);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  try {
    await fn(base, store);
  } finally {
    server.closeAllConnections?.();
    await new Promise((resolve) => server.close(resolve));
  }
}

const json = (body, method = "POST") => ({ method, headers: { "content-type": "application/json" }, body: JSON.stringify(body) });

test("starts empty and lists todos as JSON", async () => {
  await withServer(async (base) => {
    const res = await fetch(`${base}/todos`);
    assert.equal(res.status, 200);
    assert.match(res.headers.get("content-type"), /^application\/json/);
    assert.deepEqual(await res.json(), []);
  });
});

test("POST creates a todo and trims the title", async () => {
  await withServer(async (base) => {
    const res = await fetch(`${base}/todos`, json({ title: "  write tests " }));
    assert.equal(res.status, 201);
    assert.deepEqual(await res.json(), { id: 1, title: "write tests", done: false });
    const list = await (await fetch(`${base}/todos`)).json();
    assert.equal(list.length, 1);
  });
});

test("POST validates the title and the JSON body", async () => {
  await withServer(async (base) => {
    for (const bad of [{}, { title: "" }, { title: "   " }, { title: 5 }, { title: null }]) {
      const res = await fetch(`${base}/todos`, json(bad));
      assert.equal(res.status, 400, JSON.stringify(bad));
      assert.equal(typeof (await res.json()).error, "string");
    }
    const broken = await fetch(`${base}/todos`, { method: "POST", headers: { "content-type": "application/json" }, body: "{nope" });
    assert.equal(broken.status, 400);
    assert.equal(typeof (await broken.json()).error, "string");
    const arr = await fetch(`${base}/todos`, json([1, 2]));
    assert.equal(arr.status, 400);
  });
});

test("oversized bodies are rejected with 413", async () => {
  await withServer(async (base) => {
    const res = await fetch(`${base}/todos`, json({ title: "x".repeat(20_000) }));
    assert.equal(res.status, 413);
  });
});

test("GET /todos/:id returns the todo, 404 for unknown and 400 for bad ids", async () => {
  await withServer(async (base, store) => {
    store.create({ title: "a" });
    const ok = await fetch(`${base}/todos/1`);
    assert.equal(ok.status, 200);
    assert.deepEqual(await ok.json(), { id: 1, title: "a", done: false });
    assert.equal((await fetch(`${base}/todos/99`)).status, 404);
    for (const bad of ["abc", "0", "-1", "1.5"]) {
      assert.equal((await fetch(`${base}/todos/${bad}`)).status, 400, bad);
    }
  });
});

test("PATCH updates only the allowed fields", async () => {
  await withServer(async (base, store) => {
    store.create({ title: "a" });
    const res = await fetch(`${base}/todos/1`, json({ done: true, id: 42, extra: "x" }, "PATCH"));
    assert.equal(res.status, 200);
    assert.deepEqual(await res.json(), { id: 1, title: "a", done: true });
    const renamed = await fetch(`${base}/todos/1`, json({ title: " b " }, "PATCH"));
    assert.deepEqual(await renamed.json(), { id: 1, title: "b", done: true });
    assert.equal((await fetch(`${base}/todos/1`, json({ done: "yes" }, "PATCH"))).status, 400);
    assert.equal((await fetch(`${base}/todos/1`, json({ title: " " }, "PATCH"))).status, 400);
    assert.equal((await fetch(`${base}/todos/7`, json({ done: true }, "PATCH"))).status, 404);
  });
});

test("DELETE removes a todo and returns 204 without a body", async () => {
  await withServer(async (base, store) => {
    store.create({ title: "a" });
    const res = await fetch(`${base}/todos/1`, { method: "DELETE" });
    assert.equal(res.status, 204);
    assert.equal(await res.text(), "");
    assert.equal((await fetch(`${base}/todos/1`, { method: "DELETE" })).status, 404);
  });
});

test("unknown paths are 404 and unsupported methods are 405 with an allow header", async () => {
  await withServer(async (base) => {
    const missing = await fetch(`${base}/nope`);
    assert.equal(missing.status, 404);
    assert.equal(typeof (await missing.json()).error, "string");
    const put = await fetch(`${base}/todos`, { method: "PUT" });
    assert.equal(put.status, 405);
    assert.match(put.headers.get("allow"), /GET/);
    assert.match(put.headers.get("allow"), /POST/);
    assert.equal((await fetch(`${base}/todos/1`, { method: "POST" })).status, 405);
  });
});

test("query strings are ignored when routing", async () => {
  await withServer(async (base) => {
    assert.equal((await fetch(`${base}/todos?limit=5`)).status, 200);
  });
});
