import test from "node:test";
import assert from "node:assert/strict";
import { SearchableList } from "../src/SearchableList";
import { click, mount, text, type } from "./helpers";

const FRUIT = ["Apple", "apricot", "Banana", "Blueberry", "Cherry"];

const input = (c: HTMLElement) => c.querySelector('input[aria-label="search"]') as HTMLInputElement;
const items = (c: HTMLElement) => Array.from(c.querySelectorAll("ul > li")).map((li) => text(li));
const count = (c: HTMLElement) => text(c.querySelector('[data-testid="count"]'));

test("renders every item, the placeholder and the count", () => {
  const m = mount(<SearchableList items={FRUIT} />);
  assert.deepEqual(items(m.container), FRUIT);
  assert.equal(input(m.container).placeholder, "Search...");
  assert.equal(count(m.container), "5 of 5 shown");
  assert.equal(m.container.querySelector('button[aria-label="clear"]'), null);
  m.unmount();
});

test("custom placeholder", () => {
  const m = mount(<SearchableList items={FRUIT} placeholder="Find fruit" />);
  assert.equal(input(m.container).placeholder, "Find fruit");
  m.unmount();
});

test("filters case-insensitively, keeps order and updates the count", () => {
  const m = mount(<SearchableList items={FRUIT} />);
  type(input(m.container), "AP");
  assert.deepEqual(items(m.container), ["Apple", "apricot"]);
  assert.equal(count(m.container), "2 of 5 shown");
  type(input(m.container), "  rr ");
  assert.deepEqual(items(m.container), ["Blueberry", "Cherry"]);
  m.unmount();
});

test("blank queries show everything", () => {
  const m = mount(<SearchableList items={FRUIT} />);
  type(input(m.container), "   ");
  assert.equal(items(m.container).length, 5);
  assert.equal(m.container.querySelector('button[aria-label="clear"]') === null, false, "clear is shown for any non-empty input value");
  m.unmount();
});

test("shows an empty state instead of the list", () => {
  const m = mount(<SearchableList items={FRUIT} />);
  type(input(m.container), " zzz ");
  assert.equal(m.container.querySelector("ul"), null);
  assert.equal(text(m.container.querySelector('[data-testid="empty"]')), 'No results for "zzz"');
  assert.equal(count(m.container), "0 of 5 shown");
  m.unmount();
});

test("clear button resets the query", () => {
  const m = mount(<SearchableList items={FRUIT} />);
  type(input(m.container), "ban");
  assert.deepEqual(items(m.container), ["Banana"]);
  click(m.container.querySelector('button[aria-label="clear"]')!);
  assert.equal(input(m.container).value, "");
  assert.equal(items(m.container).length, 5);
  assert.equal(m.container.querySelector('button[aria-label="clear"]'), null);
  m.unmount();
});

test("clicking an item calls onSelect with that item", () => {
  const picked: string[] = [];
  const m = mount(<SearchableList items={FRUIT} onSelect={(i) => picked.push(i)} />);
  type(input(m.container), "ch");
  click(m.container.querySelector("li")!);
  assert.deepEqual(picked, ["Cherry"]);
  m.unmount();
});

test("duplicates are all rendered without React key warnings", () => {
  const errors: unknown[][] = [];
  const original = console.error;
  console.error = (...args: unknown[]) => errors.push(args);
  try {
    const m = mount(<SearchableList items={["a", "b", "a", "a"]} />);
    assert.deepEqual(items(m.container), ["a", "b", "a", "a"]);
    type(input(m.container), "a");
    assert.deepEqual(items(m.container), ["a", "a", "a"]);
    m.unmount();
  } finally {
    console.error = original;
  }
  assert.deepEqual(errors, [], "React logged: " + JSON.stringify(errors));
});

test("works with no items", () => {
  const m = mount(<SearchableList items={[]} />);
  assert.equal(count(m.container), "0 of 0 shown");
  m.unmount();
});
