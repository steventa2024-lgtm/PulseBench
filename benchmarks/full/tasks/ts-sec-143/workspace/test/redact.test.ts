import test from "node:test";
import assert from "node:assert/strict";
import { REDACTED, redact, redactUrl } from "../src/redact";

const R = REDACTED;

test("sensitive keys are masked at any depth, case-insensitively", () => {
  const out = redact({
    user: "ada",
    password: "hunter2",
    nested: { accessToken: "abc", "X-Api-Key": "k", note: "fine", deeper: [{ Authorization: "Bearer zzz", ok: 1 }] },
    "Set-Cookie": ["a=b", "c=d"],
    privateKey: { n: 1 },
    sessionId: "s",
  });
  assert.deepEqual(out, {
    user: "ada",
    password: R,
    nested: { accessToken: R, "X-Api-Key": R, note: "fine", deeper: [{ Authorization: R, ok: 1 }] },
    "Set-Cookie": R,
    privateKey: R,
    sessionId: R,
  });
});

test("the input is never mutated", () => {
  const input = { password: "p", nested: { token: "t" }, list: [{ secret: "s" }] };
  const snapshot = JSON.stringify(input);
  redact(input);
  assert.equal(JSON.stringify(input), snapshot);
});

test("strings are scanned for bearer tokens and key=value pairs", () => {
  assert.equal(redact("Authorization failed for Bearer abc.def-123"), `Authorization failed for Bearer ${R}`);
  assert.equal(redact("connect password=hunter2 user=ada"), `connect password=${R} user=ada`);
  assert.equal(redact("a=1&token=xyz&b=2"), `a=1&token=${R}&b=2`);
  assert.equal(redact("api_key: sk-123; other: 5"), `api_key: ${R}; other: 5`);
  assert.equal(redact("nothing to see"), "nothing to see");
});

test("strings inside structures are scanned too", () => {
  assert.deepEqual(redact({ msg: "retry with password=abc", tags: ["Bearer t0k3n"] }), { msg: `retry with password=${R}`, tags: [`Bearer ${R}`] });
});

test("primitives pass through, odd types are labelled", () => {
  assert.equal(redact(5), 5);
  assert.equal(redact(true), true);
  assert.equal(redact(null), null);
  assert.equal(redact(undefined), undefined);
  assert.equal(redact(10n), "10");
  assert.equal(redact(() => 1), "[Function]");
  assert.equal(redact(Symbol("s")), "[Symbol]");
});

test("dates are preserved as copies", () => {
  const d = new Date("2024-01-02T03:04:05Z");
  const out = redact({ at: d }) as { at: Date };
  assert.ok(out.at instanceof Date);
  assert.equal(out.at.toISOString(), d.toISOString());
  assert.notEqual(out.at, d);
});

test("errors keep name and message but lose the stack", () => {
  const err = new TypeError("bad password=pw1");
  const out = redact({ err }) as { err: Record<string, unknown> };
  assert.deepEqual(out.err, { name: "TypeError", message: `bad password=${R}` });
});

test("circular references and deep nesting are handled", () => {
  const a: Record<string, unknown> = { name: "a" };
  a.self = a;
  a.list = [a];
  assert.deepEqual(redact(a), { name: "a", self: "[Circular]", list: ["[Circular]"] });

  let deep: Record<string, unknown> = { leaf: true };
  for (let i = 0; i < 12; i++) deep = { child: deep };
  let cursor = redact(deep) as Record<string, unknown>;
  let depth = 0;
  while (typeof cursor === "object" && cursor !== null && "child" in cursor) {
    cursor = cursor.child as Record<string, unknown>;
    depth++;
  }
  assert.equal(cursor as unknown, "[MaxDepth]");
  assert.ok(depth <= 9);
});

test("shared (non-circular) references are copied normally", () => {
  const shared = { v: 1 };
  assert.deepEqual(redact({ a: shared, b: shared }), { a: { v: 1 }, b: { v: 1 } });
});

test("keys that merely resemble sensitive words are not over-masked", () => {
  assert.deepEqual(redact({ author: "x", tokenizer: "y" }) as object, { author: "x", tokenizer: R });
});

test("redactUrl masks userinfo passwords and sensitive query parameters", () => {
  assert.equal(redactUrl("https://ada:hunter2@example.com/path?q=1&token=abc&Api_Key=zzz#frag"), `https://ada:${R}@example.com/path?q=1&token=${R}&Api_Key=${R}#frag`);
  assert.equal(redactUrl("https://TOKEN123@example.com/x"), `https://${R}@example.com/x`);
  assert.equal(redactUrl("https://example.com/a/b?page=2"), "https://example.com/a/b?page=2");
});

test("redactUrl falls back to string scanning for non-URLs and never throws", () => {
  assert.equal(redactUrl("not a url password=abc"), `not a url password=${R}`);
  assert.equal(redactUrl(""), "");
});
