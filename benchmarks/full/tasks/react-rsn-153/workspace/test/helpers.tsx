import "./dom";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";

export interface Mounted {
  container: HTMLElement;
  root: Root;
  unmount(): void;
}

export function mount(element: ReactElement): Mounted {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  act(() => root.render(element));
  return {
    container,
    root,
    unmount() {
      act(() => root.unmount());
      container.remove();
    },
  };
}

export function click(el: Element): void {
  act(() => {
    el.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
}

/** Type into a controlled <input> the way a user would. */
export function type(input: HTMLInputElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  act(() => {
    setter.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

export function text(el: Element | null): string {
  return (el?.textContent ?? "").trim();
}
