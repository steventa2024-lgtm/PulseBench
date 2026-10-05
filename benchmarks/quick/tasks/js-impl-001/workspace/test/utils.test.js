"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { chunk, groupBy, uniqueBy, pick } = require("../src/utils");

test("chunk splits into equal parts and a shorter tail", () => {
  assert.deepEqual(chunk([1, 2, 3, 4, 5], 2), [[1, 2], [3, 4], [5]]);
  assert.deepEqual(chunk([1, 2, 3], 3), [[1, 2, 3]]);
  assert.deepEqual(chunk([], 3), []);
});

test("chunk validates size and does not mutate", () => {
  const input = [1, 2, 3];
  chunk(input, 2);
  assert.deepEqual(input, [1, 2, 3]);
  for (const bad of [0, -1, 1.5, NaN, "2"]) {
    assert.throws(() => chunk(input, bad), RangeError);
  }
});

test("groupBy keeps order and uses plain objects", () => {
  const out = groupBy(["apple", "avocado", "banana", "blueberry", "cherry"], (s) => s[0]);
  assert.deepEqual(out, { a: ["apple", "avocado"], b: ["banana", "blueberry"], c: ["cherry"] });
  assert.equal(Object.getPrototypeOf(out), Object.prototype);
  assert.deepEqual(groupBy([], (x) => x), {});
});

test("groupBy handles keys that collide with Object.prototype members", () => {
  const out = groupBy(["constructor", "toString", "constructor"], (s) => s);
  assert.deepEqual(out.constructor, ["constructor", "constructor"]);
  assert.deepEqual(out.toString, ["toString"]);
});

test("uniqueBy keeps the first occurrence", () => {
  const rows = [{ id: 1, v: "a" }, { id: 2, v: "b" }, { id: 1, v: "c" }, { id: 3, v: "d" }, { id: 2, v: "e" }];
  assert.deepEqual(uniqueBy(rows, (r) => r.id), [rows[0], rows[1], rows[3]]);
  assert.deepEqual(uniqueBy([3, 1, 3, 2, 1], (x) => x), [3, 1, 2]);
});

test("pick copies only own, existing keys", () => {
  const proto = { inherited: 1 };
  const obj = Object.assign(Object.create(proto), { a: 1, b: undefined, c: 3 });
  assert.deepEqual(pick(obj, ["a", "b", "inherited", "zzz"]), { a: 1, b: undefined });
  assert.ok(!("zzz" in pick(obj, ["zzz"])));
  assert.deepEqual(pick({ a: 1 }, []), {});
});
