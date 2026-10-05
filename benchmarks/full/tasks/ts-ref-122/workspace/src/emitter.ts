/* eslint-disable @typescript-eslint/no-explicit-any */

/** A tiny synchronous event emitter. */
export class Emitter {
  private handlers: Record<string, Function[]> = {};

  /** Register a handler. Returns a function that removes it. */
  on(name: string, handler: (payload: any) => void): () => void {
    (this.handlers[name] ??= []).push(handler);
    return () => this.off(name, handler);
  }

  /** Register a handler that runs at most once. */
  once(name: string, handler: (payload: any) => void): () => void {
    const wrapper = (payload: any) => {
      this.off(name, wrapper);
      handler(payload);
    };
    return this.on(name, wrapper);
  }

  /** Remove one handler (no-op if it is not registered). */
  off(name: string, handler: Function): void {
    const list = this.handlers[name];
    if (!list) return;
    const index = list.indexOf(handler);
    if (index >= 0) list.splice(index, 1);
  }

  /** Call every handler registered for `name`, in registration order. */
  emit(name: string, payload?: any): void {
    for (const handler of [...(this.handlers[name] ?? [])]) {
      handler(payload);
    }
  }

  listenerCount(name: string): number {
    return this.handlers[name]?.length ?? 0;
  }
}
