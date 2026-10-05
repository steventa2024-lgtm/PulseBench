import test from "node:test";
import assert from "node:assert/strict";
import { MinStack } from "../src/minstack";

test("push and top", () => {
  const stack = new MinStack<number>();
  stack.push(3);
  stack.push(5);
  assert.equal(stack.top(), 5);
  assert.equal(stack.size(), 2);
});

test("min tracks the smallest element", () => {
  const stack = new MinStack<number>();
  [5, 3, 7].forEach((n) => stack.push(n));
  assert.equal(stack.min(), 5);
  stack.pop();
  assert.equal(stack.min(), "3");
});

test("pop on an empty stack throws", () => {
  const stack = new MinStack<number>();
  assert.throws(() => stack.pop());
});

test("clear empties the stack", () => {
  const stack = new MinStack<string>();
  stack.push("a");
  stack.clear();
  assert.equal(stack.isEmpty, true);
  assert.equal(stack.peek(), null);
});

test("custom comparator", () => {
  const stack = new MinStack<string>((a, b) => a.length - b.length);
  stack.push("ccc");
  stack.push("a");
  assert.equal(stack.min(), "ccc");
});
