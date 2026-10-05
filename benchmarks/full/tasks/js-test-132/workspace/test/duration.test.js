"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { parseDuration, formatDuration } = require("../src/duration");

test("parses seconds", () => {
  assert.equal(parseDuration("90s"), 90_000);
});

test("formats minutes", () => {
  assert.equal(formatDuration(60_000), "1m");
});
