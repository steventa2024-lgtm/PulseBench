type Handler<P> = (payload: P) => void;
type EmitArgs<P> = [P] extends [undefined] ? [] : [payload: P];

/** A tiny synchronous event emitter. */
export class Emitter<Events extends Record<string, unknown>> {
  private handlers: { [K in keyof Events]?: Array<Handler<Events[K]>> } = {};

  on<K extends keyof Events>(name: K, handler: Handler<Events[K]>): () => void {
    (this.handlers[name] ??= []).push(handler);
    return () => this.off(name, handler);
  }

  once<K extends keyof Events>(name: K, handler: Handler<Events[K]>): () => void {
    const wrapper: Handler<Events[K]> = (payload) => {
      this.off(name, wrapper);
      handler(payload);
    };
    return this.on(name, wrapper);
  }

  off<K extends keyof Events>(name: K, handler: Handler<Events[K]>): void {
    const list = this.handlers[name];
    if (!list) return;
    const index = list.indexOf(handler);
    if (index >= 0) list.splice(index, 1);
  }

  emit<K extends keyof Events>(name: K, ...payload: EmitArgs<Events[K]>): void {
    for (const handler of [...(this.handlers[name] ?? [])]) {
      handler(payload[0] as Events[K]);
    }
  }

  listenerCount<K extends keyof Events>(name: K): number {
    return this.handlers[name]?.length ?? 0;
  }
}
