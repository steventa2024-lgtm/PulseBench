import test from "node:test";
import assert from "node:assert/strict";
import { mapLimit, retry } from "../src/async";

test("retry returns the first successful result", async () => {
  let calls = 0;
  const result = await retry(async () => {
    calls++;
    return "ok";
  }, 3, 1);
  assert.equal(result, "ok");
  assert.equal(calls, 1);
});

test("retry retries rejected promises and eventually succeeds", async () => {
  let calls = 0;
  const result = await retry(async () => {
    calls++;
    if (calls < 3) throw new Error("flaky " + calls);
    return calls;
  }, 3, 1);
  assert.equal(result, 3);
});

test("retry makes exactly `attempts` calls then rethrows the last error", async () => {
  let calls = 0;
  await assert.rejects(
    retry(async () => {
      calls++;
      throw new Error("fail " + calls);
    }, 4, 1),
    /fail 4/,
  );
  assert.equal(calls, 4);
});

test("retry waits between attempts but not after the last one", async () => {
  const stamps: number[] = [];
  await assert.rejects(
    retry(async () => {
      stamps.push(Date.now());
      throw new Error("x");
    }, 3, 40),
  );
  assert.equal(stamps.length, 3);
  assert.ok(stamps[1] - stamps[0] >= 35, "should wait before attempt 2");
  assert.ok(stamps[2] - stamps[1] >= 35, "should wait before attempt 3");
});

test("retry works with a single attempt", async () => {
  await assert.rejects(retry(async () => { throw new Error("once"); }, 1, 1), /once/);
});

test("mapLimit preserves order", async () => {
  const out = await mapLimit([30, 5, 20, 1], 2, async (ms, i) => {
    await new Promise((r) => setTimeout(r, ms));
    return `${i}:${ms}`;
  });
  assert.deepEqual(out, ["0:30", "1:5", "2:20", "3:1"]);
});

test("mapLimit never exceeds the concurrency limit but does use it", async () => {
  let running = 0;
  let peak = 0;
  await mapLimit([1, 2, 3, 4, 5, 6, 7, 8], 3, async () => {
    running++;
    peak = Math.max(peak, running);
    await new Promise((r) => setTimeout(r, 15));
    running--;
  });
  assert.equal(peak, 3);
});

test("mapLimit rejects when a task rejects and handles empty input", async () => {
  await assert.rejects(mapLimit([1, 2, 3], 2, async (n) => { if (n === 2) throw new Error("boom"); return n; }), /boom/);
  assert.deepEqual(await mapLimit([], 3, async (x) => x), []);
});
