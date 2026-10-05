import test from "node:test";
import assert from "node:assert/strict";
import { Emitter } from "../src/emitter";

type AppEvents = {
  login: { user: string };
  logout: undefined;
  count: number;
};

test("handlers receive typed payloads in registration order", () => {
  const bus = new Emitter<AppEvents>();
  const seen: string[] = [];
  bus.on("login", (p) => seen.push("1:" + p.user.toUpperCase()));
  bus.on("login", (p) => seen.push("2:" + p.user));
  bus.emit("login", { user: "ada" });
  assert.deepEqual(seen, ["1:ADA", "2:ada"]);
});

test("events without a payload are emitted without an argument", () => {
  const bus = new Emitter<AppEvents>();
  let calls = 0;
  bus.on("logout", () => calls++);
  bus.emit("logout");
  assert.equal(calls, 1);
});

test("once runs a single time", () => {
  const bus = new Emitter<AppEvents>();
  const seen: number[] = [];
  bus.once("count", (n) => seen.push(n));
  bus.emit("count", 1);
  bus.emit("count", 2);
  assert.deepEqual(seen, [1]);
  assert.equal(bus.listenerCount("count"), 0);
});

test("the function returned by on removes the handler; off with an unknown handler is harmless", () => {
  const bus = new Emitter<AppEvents>();
  let calls = 0;
  const handler = () => calls++;
  const off = bus.on("count", handler);
  bus.emit("count", 1);
  off();
  bus.emit("count", 2);
  assert.equal(calls, 1);
  bus.off("count", handler);
  bus.off("login", () => {});
  assert.equal(bus.listenerCount("count"), 0);
});

test("a handler removed during emit still lets the current round finish", () => {
  const bus = new Emitter<AppEvents>();
  const seen: string[] = [];
  let offSecond = () => {};
  bus.on("count", () => {
    seen.push("first");
    offSecond();
  });
  offSecond = bus.on("count", () => seen.push("second"));
  bus.emit("count", 1);
  assert.deepEqual(seen, ["first", "second"]);
  bus.emit("count", 2);
  assert.deepEqual(seen, ["first", "second", "first"]);
});

// Compile-time checks: never executed, but `tsc` must accept them exactly as written.
function typeChecks() {
  const bus = new Emitter<AppEvents>();
  // @ts-expect-error unknown event name
  bus.emit("nope", 1);
  // @ts-expect-error wrong payload type
  bus.emit("count", "x");
  // @ts-expect-error payload is required for login
  bus.emit("login");
  // @ts-expect-error logout takes no payload
  bus.emit("logout", 1);
  // @ts-expect-error handler parameter must match the payload type
  bus.on("login", (p: number) => p);
  // @ts-expect-error unknown event name on `on`
  bus.on("missing", () => {});
  // @ts-expect-error unknown event name on `once`
  bus.once("missing", () => {});
  // @ts-expect-error unknown event name on `listenerCount`
  bus.listenerCount("missing");
}

test("compile-time checks are enforced by tsc", () => {
  assert.equal(typeof typeChecks, "function");
});
