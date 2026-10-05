import test, { afterEach, beforeEach, mock } from "node:test";
import assert from "node:assert/strict";
import { act } from "react";
import { Stopwatch } from "../src/Stopwatch";
import { click, mount, text, type Mounted } from "./helpers";

// Track every interval created/cleared through the (mocked) timer functions.
let active: Set<unknown>;
let created: number;
let realSet: typeof setInterval;
let realClear: typeof clearInterval;

beforeEach(() => {
  mock.timers.enable({ apis: ["setInterval"] });
  active = new Set();
  created = 0;
  realSet = globalThis.setInterval;
  realClear = globalThis.clearInterval;
  globalThis.setInterval = ((...args: Parameters<typeof setInterval>) => {
    const id = realSet(...args);
    active.add(id);
    created++;
    return id;
  }) as typeof setInterval;
  globalThis.clearInterval = ((id: Parameters<typeof clearInterval>[0]) => {
    active.delete(id);
    return realClear(id);
  }) as typeof clearInterval;
});

afterEach(() => {
  globalThis.setInterval = realSet;
  globalThis.clearInterval = realClear;
  mock.timers.reset();
});

const time = (m: Mounted) => text(m.container.querySelector('[data-testid="time"]'));
const toggle = (m: Mounted) => click(m.container.querySelector('[data-testid="toggle"]')!);
const reset = (m: Mounted) => click(m.container.querySelector('[data-testid="reset"]')!);
const tick = (ms: number) => act(() => mock.timers.tick(ms));

test("starts at 0s and does not run until started", () => {
  const m = mount(<Stopwatch label="Lap" />);
  assert.equal(time(m), "Lap: 0s");
  tick(5000);
  assert.equal(time(m), "Lap: 0s");
  assert.equal(active.size, 0);
  m.unmount();
});

test("counts one second per tick with exactly one interval", () => {
  const m = mount(<Stopwatch label="Lap" />);
  toggle(m);
  assert.equal(active.size, 1);
  tick(3000);
  assert.equal(time(m), "Lap: 3s");
  assert.equal(active.size, 1, "re-renders must not create more intervals");
  assert.equal(created, 1, "the interval must not be restarted by re-renders");
  m.unmount();
});

test("stop pauses counting and clears the interval; start resumes from where it was", () => {
  const m = mount(<Stopwatch label="Lap" />);
  toggle(m);
  tick(2000);
  toggle(m);
  assert.equal(active.size, 0);
  tick(10_000);
  assert.equal(time(m), "Lap: 2s");
  toggle(m);
  tick(1000);
  assert.equal(time(m), "Lap: 3s");
  m.unmount();
});

test("reset sets the time back to zero while keeping the running state", () => {
  const m = mount(<Stopwatch label="Lap" />);
  toggle(m);
  tick(4000);
  reset(m);
  assert.equal(time(m), "Lap: 0s");
  tick(2000);
  assert.equal(time(m), "Lap: 2s");
  m.unmount();
});

test("a parent re-render with new props does not disturb the timer", () => {
  const m = mount(<Stopwatch label="A" />);
  toggle(m);
  tick(500);
  act(() => m.root.render(<Stopwatch label="B" />));
  tick(500);
  assert.equal(time(m), "B: 1s", "the 1s tick must still fire on schedule after a re-render");
  assert.equal(created, 1);
  m.unmount();
});

test("unmounting clears the interval", () => {
  const m = mount(<Stopwatch label="Lap" />);
  toggle(m);
  assert.equal(active.size, 1);
  m.unmount();
  assert.equal(active.size, 0);
});

test("rapid toggling never leaves more than one interval", () => {
  const m = mount(<Stopwatch label="Lap" />);
  for (let i = 0; i < 5; i++) toggle(m);
  assert.ok(active.size <= 1);
  tick(1000);
  assert.equal(time(m), "Lap: 1s");
  m.unmount();
});
