import type { ReactNode } from "react";
import { Dot } from "../components/ui";
import { cx, mb } from "../lib/format";
import { useApp } from "../store/app";
import { navItems, useNav, type Route } from "../store/nav";

function Wordmark() {
  return (
    <div className="flex items-center gap-2.5">
      <svg width="22" height="22" viewBox="0 0 32 32" aria-hidden>
        <rect width="32" height="32" rx="6" fill="#0b1220" stroke="#2a3a63" />
        <polyline points="3,17 10,17 14,7 19,25 23,13 26,17 29,17" fill="none" stroke="#22d3ee" strokeWidth="2.4" strokeLinejoin="round" strokeLinecap="round" />
      </svg>
      <span className="text-[13px] font-bold tracking-[0.22em] text-text">PULSEBENCH</span>
    </div>
  );
}

function StatusBar() {
  const system = useApp((s) => s.system);
  const providers = useApp((s) => s.providers);
  const gpu = system?.gpus[0];
  return (
    <div className="flex items-center gap-4 text-[11.5px] text-muted">
      {gpu ? (
        <span className="num" title={gpu.driverVersion ?? undefined}>
          {gpu.name}
          {gpu.vramTotalMb !== null && <span className="text-dim"> · {mb(gpu.vramTotalMb)}</span>}
        </span>
      ) : system ? (
        <span>no GPU detected</span>
      ) : (
        <span className="text-dim">detecting hardware…</span>
      )}
      {(providers ?? []).map((p) => (
        <span key={p.providerId} className="flex items-center gap-1.5" title={p.error ?? `${p.models.length} models`}>
          {p.name}
          <Dot state={p.state === "connected" ? "ok" : p.state === "disabled" ? "off" : "bad"} />
        </span>
      ))}
    </div>
  );
}

function isActive(item: Route["page"], current: Route["page"]): boolean {
  return item === current || (item === "results" && current === "model") || (item === "new" && current === "live");
}

export function Shell({ children }: { children: ReactNode }) {
  const route = useNav((s) => s.route);
  const go = useNav((s) => s.go);
  const live = useApp((s) => s.live);
  const running = live && !live.finished;
  return (
    <div className="flex h-full flex-col">
      <header className="flex h-11 shrink-0 items-center justify-between border-b border-line bg-panel px-4">
        <Wordmark />
        <StatusBar />
      </header>
      <div className="flex min-h-0 flex-1">
        <nav className="flex w-[188px] shrink-0 flex-col gap-px border-r border-line bg-panel py-3" aria-label="Main">
          {navItems.map((item) => (
            <button
              key={item.page}
              onClick={() => go(item.route)}
              className={cx(
                "flex items-center justify-between border-l-2 px-4 py-1.5 text-left text-[12.5px]",
                isActive(item.page, route.page) ? "border-accent bg-accent/10 text-text" : "border-transparent text-muted hover:bg-panel2 hover:text-text",
              )}
            >
              {item.label}
            </button>
          ))}
          {running && (
            <button onClick={() => go({ page: "live" })} className={cx("mx-3 mt-3 flex items-center gap-2 rounded-sm border border-accent/50 bg-accent/10 px-2 py-1.5 text-left text-[11.5px] text-accent", route.page === "live" && "border-accent")}>
              <span className="live-dot h-1.5 w-1.5 rounded-full bg-accent2" /> Benchmark running
            </button>
          )}
        </nav>
        <main className="min-w-0 flex-1 overflow-y-auto bg-ink p-5">{children}</main>
      </div>
      <footer className="flex h-7 shrink-0 items-center justify-between border-t border-line bg-panel px-4 text-[11px] text-dim">
        <span>Built by ZeroPulse</span>
        <span>Local-first · no telemetry · your code never leaves this machine</span>
      </footer>
    </div>
  );
}
