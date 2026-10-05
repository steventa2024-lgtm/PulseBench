import { JSDOM } from "jsdom";

// Minimal browser environment for React tests (loaded before react-dom).
const dom = new JSDOM("<!doctype html><html><body></body></html>", { url: "http://localhost/" });
const win = dom.window as unknown as Record<string, unknown>;
for (const key of ["window", "document", "navigator", "HTMLElement", "HTMLInputElement", "Node", "Event", "MouseEvent", "KeyboardEvent"]) {
  Object.defineProperty(globalThis, key, { value: win[key], configurable: true, writable: true });
}
(globalThis as Record<string, unknown>).IS_REACT_ACT_ENVIRONMENT = true;
