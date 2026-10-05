import test from "node:test";
import assert from "node:assert/strict";
import { MinStack } from "../src/minStack";

test("push, peek and size", () => {
  const stack = new MinStack<number>();
  stack.push(3);
  stack.push(5);
  assert.equal(stack.peek(), 5);
  assert.equal(stack.size, 2);
  assert.equal(stack.isEmpty(), false);
});

test("min tracks the smallest element through pushes and pops", () => {
  const stack = new MinStack<number>();
  [5, 3, 7].forEach((n) => stack.push(n));
  assert.equal(stack.min(), 3);
  assert.equal(stack.pop(), 7);
  assert.equal(stack.min(), 3);
  assert.equal(stack.pop(), 3);
  assert.equal(stack.min(), 5);
});

test("duplicate minimums are tracked", () => {
  const stack = new MinStack<number>();
  stack.push(2);
  stack.push(2);
  stack.push(4);
  stack.pop();
  stack.pop();
  assert.equal(stack.min(), 2);
  stack.pop();
  assert.equal(stack.min(), undefined);
});

test("pop, peek and min on an empty stack return undefined", () => {
  const stack = new MinStack<number>();
  assert.equal(stack.pop(), undefined);
  assert.equal(stack.peek(), undefined);
  assert.equal(stack.min(), undefined);
  assert.equal(stack.isEmpty(), true);
  assert.equal(stack.size, 0);
});

test("peek does not remove the element", () => {
  const stack = new MinStack<string>();
  stack.push("a");
  assert.equal(stack.peek(), "a");
  assert.equal(stack.peek(), "a");
  assert.equal(stack.size, 1);
});

test("clear empties the stack and forgets the minimum", () => {
  const stack = new MinStack<number>();
  stack.push(1);
  stack.push(2);
  stack.clear();
  assert.equal(stack.isEmpty(), true);
  assert.equal(stack.peek(), undefined);
  stack.push(9);
  assert.equal(stack.min(), 9);
});

test("custom comparator defines the ordering", () => {
  // By length, "b" is smaller than "aa" even though "aa" sorts first alphabetically.
  const stack = new MinStack<string>((a, b) => a.length - b.length);
  stack.push("b");
  stack.push("aa");
  stack.push("cccc");
  assert.equal(stack.min(), "b");
  stack.pop();
  stack.pop();
  assert.equal(stack.min(), "b");
});
