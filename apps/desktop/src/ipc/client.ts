import type { RunEvent } from "@pulsebench/types";

/** How the UI talks to the Rust side: the Tauri shell, or the headless HTTP bridge used in dev/tests. */
export interface Transport {
  readonly kind: "tauri" | "bridge";
  call<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  subscribe(handler: (event: RunEvent) => void): Promise<() => void>;
}

export class BackendError extends Error {}

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function bridgeUrl(): string {
  const fromQuery = new URLSearchParams(window.location.search).get("bridge");
  return (fromQuery ?? import.meta.env.VITE_BRIDGE_URL ?? "http://127.0.0.1:8787").replace(/\/$/, "");
}

async function tauriTransport(): Promise<Transport> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  return {
    kind: "tauri",
    async call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
      try {
        return (await invoke("rpc", { cmd, args })) as T;
      } catch (e) {
        throw new BackendError(typeof e === "string" ? e : String(e));
      }
    },
    async subscribe(handler) {
      const un = await listen<RunEvent>("pulsebench://event", (e) => handler(e.payload));
      return un;
    },
  };
}

function bridgeTransport(): Transport {
  const base = bridgeUrl();
  return {
    kind: "bridge",
    async call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
      let res: Response;
      try {
        res = await fetch(`${base}/rpc/${cmd}`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(args) });
      } catch {
        throw new BackendError(`Cannot reach the PulseBench backend at ${base}. Start it with \`cargo run -p pulsebench-bridge\`.`);
      }
      const body = (await res.json().catch(() => null)) as { error?: string } | null;
      if (!res.ok) throw new BackendError(body?.error ?? `HTTP ${res.status}`);
      return body as T;
    },
    async subscribe(handler) {
      const es = new EventSource(`${base}/events`);
      es.onmessage = (m) => {
        try {
          handler(JSON.parse(m.data) as RunEvent);
        } catch {
          /* ignore malformed frames */
        }
      };
      return () => es.close();
    },
  };
}

let transport: Promise<Transport> | undefined;

export function getTransport(): Promise<Transport> {
  transport ??= isTauri() ? tauriTransport() : Promise.resolve(bridgeTransport());
  return transport;
}

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return (await getTransport()).call<T>(cmd, args);
}
