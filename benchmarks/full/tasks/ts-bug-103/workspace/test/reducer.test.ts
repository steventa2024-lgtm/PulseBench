import test from "node:test";
import assert from "node:assert/strict";
import { cartReducer, initialState, type CartState } from "../src/reducer";

function deepFreeze<T>(value: T): T {
  if (value && typeof value === "object") {
    Object.values(value as object).forEach(deepFreeze);
    Object.freeze(value);
  }
  return value;
}

const base: CartState = deepFreeze({ lines: [{ sku: "a", quantity: 2 }, { sku: "b", quantity: 1 }] });

test("add appends a new line with default quantity 1", () => {
  const next = cartReducer(base, { type: "add", sku: "c" });
  assert.deepEqual(next.lines, [{ sku: "a", quantity: 2 }, { sku: "b", quantity: 1 }, { sku: "c", quantity: 1 }]);
});

test("add increases an existing line without touching the old state", () => {
  const next = cartReducer(base, { type: "add", sku: "a", quantity: 3 });
  assert.deepEqual(next.lines[0], { sku: "a", quantity: 5 });
  assert.equal(base.lines[0].quantity, 2);
  assert.notEqual(next, base);
});

test("add ignores quantities below 1 and returns the same state", () => {
  assert.equal(cartReducer(base, { type: "add", sku: "a", quantity: 0 }), base);
  assert.equal(cartReducer(base, { type: "add", sku: "z", quantity: -2 }), base);
});

test("remove deletes a line; removing an absent sku is a no-op", () => {
  assert.deepEqual(cartReducer(base, { type: "remove", sku: "a" }).lines, [{ sku: "b", quantity: 1 }]);
  assert.equal(cartReducer(base, { type: "remove", sku: "nope" }), base);
});

test("setQuantity sets an exact quantity immutably", () => {
  const next = cartReducer(base, { type: "setQuantity", sku: "b", quantity: 9 });
  assert.deepEqual(next.lines[1], { sku: "b", quantity: 9 });
  assert.equal(base.lines[1].quantity, 1);
});

test("setQuantity of zero or less removes the line", () => {
  assert.deepEqual(cartReducer(base, { type: "setQuantity", sku: "a", quantity: 0 }).lines, [{ sku: "b", quantity: 1 }]);
  assert.deepEqual(cartReducer(base, { type: "setQuantity", sku: "a", quantity: -5 }).lines, [{ sku: "b", quantity: 1 }]);
});

test("setQuantity on an unknown sku or with the same quantity is a no-op", () => {
  assert.equal(cartReducer(base, { type: "setQuantity", sku: "zzz", quantity: 3 }), base);
  assert.equal(cartReducer(base, { type: "setQuantity", sku: "a", quantity: 2 }), base);
});

test("clear empties the cart; clearing an empty cart returns the same state", () => {
  assert.deepEqual(cartReducer(base, { type: "clear" }).lines, []);
  assert.equal(cartReducer(initialState, { type: "clear" }), initialState);
});

test("the shared initial state is never mutated", () => {
  cartReducer(initialState, { type: "add", sku: "x" });
  assert.deepEqual(initialState.lines, []);
});
