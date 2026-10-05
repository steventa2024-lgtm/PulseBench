"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { validateUser } = require("../src/validate");

const ok = { name: "Ada Lovelace", email: "ada@example.com" };
const fields = (r) => r.errors.map((e) => e.field);

test("accepts a minimal valid payload and a fully populated one", () => {
  assert.deepEqual(validateUser(ok), { valid: true, errors: [] });
  assert.equal(validateUser({ ...ok, age: 36, role: "admin" }).valid, true);
});

test("rejects non-objects", () => {
  for (const bad of [null, [], "x", 5, undefined, true]) {
    const r = validateUser(bad);
    assert.equal(r.valid, false, String(bad));
    assert.deepEqual(r.errors, [{ field: "$", message: "payload must be an object" }]);
  }
});

test("name must be a non-blank string of at most 50 characters", () => {
  assert.deepEqual(fields(validateUser({ ...ok, name: "" })), ["name"]);
  assert.deepEqual(fields(validateUser({ ...ok, name: "   " })), ["name"]);
  assert.deepEqual(fields(validateUser({ ...ok, name: 42 })), ["name"]);
  assert.deepEqual(fields(validateUser({ ...ok, name: "x".repeat(51) })), ["name"]);
  assert.equal(validateUser({ ...ok, name: "x".repeat(50) }).valid, true);
  assert.equal(validateUser({ ...ok, name: " x " }).valid, true);
  assert.deepEqual(fields(validateUser({ email: ok.email })), ["name"]);
});

test("email must look like local@domain.tld", () => {
  for (const bad of ["a@b", "a b@c.com", "@c.com", "a@.com", "plain", "", 7, null]) {
    assert.deepEqual(fields(validateUser({ ...ok, email: bad })), ["email"], String(bad));
  }
  assert.equal(validateUser({ ...ok, email: "a.b+tag@sub.example.org" }).valid, true);
  assert.deepEqual(fields(validateUser({ name: ok.name })), ["email"]);
});

test("age is optional but must be an integer within 13..120", () => {
  assert.equal(validateUser({ ...ok, age: 13 }).valid, true);
  assert.equal(validateUser({ ...ok, age: 120 }).valid, true);
  for (const bad of [12, 121, 0, -5, 20.5, "30", NaN, null]) {
    assert.deepEqual(fields(validateUser({ ...ok, age: bad })), ["age"], String(bad));
  }
});

test("role must be user or admin when present", () => {
  assert.equal(validateUser({ ...ok, role: "user" }).valid, true);
  assert.equal(validateUser({ ...ok, role: "admin" }).valid, true);
  for (const bad of ["root", "", 1, null]) {
    assert.deepEqual(fields(validateUser({ ...ok, role: bad })), ["role"], String(bad));
  }
});

test("unknown fields are rejected, including prototype-ish names", () => {
  const r = validateUser({ ...ok, admin: true, extra: 1 });
  assert.deepEqual(r.errors.filter((e) => e.message === "unknown field").map((e) => e.field), ["admin", "extra"]);
  const proto = JSON.parse('{"name":"A","email":"a@b.co","__proto__":{"x":1},"constructor":1}');
  assert.equal(validateUser(proto).valid, false);
});

test("errors come in documented order and all problems are reported", () => {
  const r = validateUser({ extra: 1, role: "x", age: 3, email: "bad", name: "" });
  assert.deepEqual(fields(r), ["name", "email", "age", "role", "extra"]);
});
