import test from "node:test";
import assert from "node:assert/strict";
import { TodoList } from "../src/TodoList";
import { click, mount, text, type } from "./helpers";

function items(container: HTMLElement): string[] {
  return Array.from(container.querySelectorAll("li")).map((li) => text(li));
}

test("renders the initial todos and the remaining count", () => {
  const m = mount(<TodoList initial={["write", "test"]} />);
  assert.deepEqual(items(m.container), ["write", "test"]);
  assert.equal(text(m.container.querySelector("p")), "2 items left");
  m.unmount();
});

test("adding a todo shows it, clears the input and updates the count", () => {
  const m = mount(<TodoList initial={["write"]} />);
  const input = m.container.querySelector("input") as HTMLInputElement;
  type(input, "  ship it  ");
  click(m.container.querySelector("button")!);
  assert.deepEqual(items(m.container), ["write", "ship it"]);
  assert.equal(input.value, "");
  assert.equal(text(m.container.querySelector("p")), "2 items left");
  m.unmount();
});

test("blank todos are ignored", () => {
  const m = mount(<TodoList />);
  type(m.container.querySelector("input") as HTMLInputElement, "   ");
  click(m.container.querySelector("button")!);
  assert.deepEqual(items(m.container), []);
  assert.equal(text(m.container.querySelector("p")), "0 items left");
  m.unmount();
});

test("toggling marks items done, decrements the count and can be undone", () => {
  const m = mount(<TodoList initial={["a", "b", "c"]} />);
  const li = () => m.container.querySelectorAll("li");
  click(li()[1]);
  assert.equal(li()[1].style.textDecoration, "line-through");
  assert.equal(text(m.container.querySelector("p")), "2 items left");
  click(li()[1]);
  assert.equal(li()[1].style.textDecoration, "none");
  assert.equal(text(m.container.querySelector("p")), "3 items left");
  m.unmount();
});

test("state is not shared between instances and props are not mutated", () => {
  const initial = ["one"];
  const a = mount(<TodoList initial={initial} />);
  type(a.container.querySelector("input") as HTMLInputElement, "two");
  click(a.container.querySelector("button")!);
  const b = mount(<TodoList initial={initial} />);
  assert.deepEqual(items(b.container), ["one"]);
  assert.deepEqual(initial, ["one"]);
  a.unmount();
  b.unmount();
});
