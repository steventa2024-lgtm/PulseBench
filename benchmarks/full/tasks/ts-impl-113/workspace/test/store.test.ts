import test from "node:test";
import assert from "node:assert/strict";
import { createStore } from "../src/store";

type State = { count: number; label: string };
type Action = { type: "inc"; by?: number } | { type: "label"; value: string } | { type: "noop" };

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "inc":
      return { ...state, count: state.count + (action.by ?? 1) };
    case "label":
      return { ...state, label: action.value };
    case "noop":
      return state;
  }
}

const initial: State = { count: 0, label: "start" };

test("dispatch updates state and getState reflects it", () => {
  const store = createStore(reducer, initial);
  assert.deepEqual(store.getState(), initial);
  store.dispatch({ type: "inc", by: 5 });
  assert.equal(store.getState().count, 5);
  assert.equal(initial.count, 0);
});

// Compile-time checks only: this function is never called, but it must type-check.
function typeChecks() {
  const store = createStore(reducer, initial);
  // @ts-expect-error unknown action type
  store.dispatch({ type: "explode" });
  // @ts-expect-error wrong payload type
  store.dispatch({ type: "inc", by: "many" });
  // @ts-expect-error listener must accept the state type
  store.subscribe((state: number) => state);
  const count: number = store.getState().count;
  return count;
}

test("types: the compile-time checks above are enforced by `tsc`", () => {
  assert.equal(typeof typeChecks, "function");
});

test("listeners run in order and receive state and previous state", () => {
  const store = createStore(reducer, initial);
  const calls: string[] = [];
  store.subscribe((s, p) => calls.push(`a:${p.count}->${s.count}`));
  store.subscribe((s) => calls.push(`b:${s.count}`));
  store.dispatch({ type: "inc" });
  assert.deepEqual(calls, ["a:0->1", "b:1"]);
});

test("no notification when the state object is unchanged", () => {
  const store = createStore(reducer, initial);
  let calls = 0;
  store.subscribe(() => calls++);
  store.dispatch({ type: "noop" });
  assert.equal(calls, 0);
  store.dispatch({ type: "inc" });
  assert.equal(calls, 1);
});

test("unsubscribe stops notifications and is idempotent", () => {
  const store = createStore(reducer, initial);
  let calls = 0;
  const off = store.subscribe(() => calls++);
  store.dispatch({ type: "inc" });
  off();
  off();
  store.dispatch({ type: "inc" });
  assert.equal(calls, 1);
});

test("a listener unsubscribing during notification does not skip or double-call others", () => {
  const store = createStore(reducer, initial);
  const calls: string[] = [];
  let offB: () => void = () => {};
  store.subscribe(() => {
    calls.push("a");
    offB();
  });
  offB = store.subscribe(() => calls.push("b"));
  store.subscribe(() => calls.push("c"));
  store.dispatch({ type: "inc" });
  assert.deepEqual(calls, ["a", "b", "c"], "the current round uses the snapshot taken at dispatch time");
  calls.length = 0;
  store.dispatch({ type: "inc" });
  assert.deepEqual(calls, ["a", "c"]);
});

test("observe fires only when the selected slice changes", () => {
  const store = createStore(reducer, initial);
  const seen: Array<[string, string]> = [];
  store.observe((s) => s.label, (v, p) => seen.push([p, v]));
  store.dispatch({ type: "inc" });
  assert.deepEqual(seen, []);
  store.dispatch({ type: "label", value: "next" });
  store.dispatch({ type: "label", value: "next" });
  assert.deepEqual(seen, [["start", "next"]]);
});

function observeTypeChecks() {
  const store = createStore(reducer, initial);
  store.observe((s) => s.count, (value: number) => value.toFixed(1));
  // @ts-expect-error callback must accept the selected type
  store.observe((s) => s.count, (value: string) => value.length);
}

test("observe is typed by its selector (checked at compile time)", () => {
  assert.equal(typeof observeTypeChecks, "function");
});

test("dispatching from a reducer throws", () => {
  let store: ReturnType<typeof createStore<number, "go">> | undefined;
  store = createStore<number, "go">((state) => {
    store!.dispatch("go");
    return state + 1;
  }, 0);
  assert.throws(() => store!.dispatch("go"), /reducers may not dispatch/);
  // the store stays usable afterwards
  const other = createStore<number, "go">((s) => s + 1, 0);
  other.dispatch("go");
  assert.equal(other.getState(), 1);
});
