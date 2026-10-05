"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { parseDuration, formatDuration } = require("../src/duration");

test("parses every unit", () => {
  assert.equal(parseDuration("5ms"), 5);
  assert.equal(parseDuration("2s"), 2000);
  assert.equal(parseDuration("3m"), 180_000);
  assert.equal(parseDuration("4h"), 14_400_000);
  assert.equal(parseDuration("1d"), 86_400_000);
});

test("ms is not confused with m followed by s", () => {
  assert.equal(parseDuration("500ms"), 500);
  assert.equal(parseDuration("1m500ms"), 60_500);
});

test("combines several units", () => {
  assert.equal(parseDuration("1h30m"), 5_400_000);
  assert.equal(parseDuration("2d3h4m5s6ms"), 2 * 86_400_000 + 3 * 3_600_000 + 4 * 60_000 + 5000 + 6);
});

test("allows whitespace between pairs and inside pairs", () => {
  assert.equal(parseDuration("1h 30m"), 5_400_000);
  assert.equal(parseDuration("  2 s  "), 2000);
  assert.equal(parseDuration("1d 1h  1m"), 86_400_000 + 3_600_000 + 60_000);
});

test("supports decimals and rounds to the nearest millisecond", () => {
  assert.equal(parseDuration("1.5h"), 5_400_000);
  assert.equal(parseDuration("0.5s"), 500);
  assert.equal(parseDuration("1.0006s"), 1001);
  assert.equal(parseDuration("1.0004s"), 1000);
});

test("repeated units add up", () => {
  assert.equal(parseDuration("1h1h"), 7_200_000);
  assert.equal(parseDuration("30s 30s"), 60_000);
});

test("zero is valid", () => {
  assert.equal(parseDuration("0s"), 0);
});

test("rejects invalid input with a TypeError", () => {
  for (const bad of ["", "   ", "10", "5x", "h", "-5s", "1h 30", "1h,30m", "abc", "1..5s", "5S", "1 h x"]) {
    assert.throws(() => parseDuration(bad), { name: "TypeError", message: "invalid duration" }, JSON.stringify(bad));
  }
  for (const bad of [undefined, null, 5, {}, []]) {
    assert.throws(() => parseDuration(bad), TypeError);
  }
});

test("formats largest unit first and omits zero components", () => {
  assert.equal(formatDuration(5_405_020), "1h 30m 5s 20ms");
  assert.equal(formatDuration(86_400_000), "1d");
  assert.equal(formatDuration(90_000), "1m 30s");
  assert.equal(formatDuration(1), "1ms");
  assert.equal(formatDuration(3_600_001), "1h 1ms");
});

test("formats zero as 0ms", () => {
  assert.equal(formatDuration(0), "0ms");
});

test("format rounds to the nearest millisecond", () => {
  assert.equal(formatDuration(1.6), "2ms");
  assert.equal(formatDuration(1.4), "1ms");
  assert.equal(formatDuration(0.4), "0ms");
});

test("format validates its input", () => {
  assert.throws(() => formatDuration(-1), RangeError);
  assert.throws(() => formatDuration(Infinity), RangeError);
  assert.throws(() => formatDuration(NaN), RangeError);
  assert.throws(() => formatDuration("5"), TypeError);
});

test("format and parse round-trip", () => {
  for (const ms of [1, 999, 1000, 61_000, 3_661_001, 90_061_001]) {
    assert.equal(parseDuration(formatDuration(ms)), ms);
  }
});
