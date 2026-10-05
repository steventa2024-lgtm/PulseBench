export const dash = "—";

export function thousands(n: number): string {
  return Math.round(n).toLocaleString("en-US");
}

export function pct(ratio: number | null | undefined, digits = 0): string {
  return ratio === null || ratio === undefined ? dash : `${(ratio * 100).toFixed(digits)}%`;
}

export function fixed(n: number | null | undefined, digits = 1, unit = ""): string {
  return n === null || n === undefined || !Number.isFinite(n) ? "n/a" : `${n.toFixed(digits)}${unit}`;
}

export function bytes(n: number | null | undefined): string {
  if (n === null || n === undefined) return dash;
  const gb = n / 1e9;
  return gb >= 1 ? `${gb.toFixed(1)} GB` : `${(n / 1e6).toFixed(0)} MB`;
}

export function mb(n: number | null | undefined): string {
  if (n === null || n === undefined) return "unavailable";
  return n >= 1024 ? `${(n / 1024).toFixed(1)} GB` : `${n} MB`;
}

export function secs(s: number | null | undefined): string {
  if (s === null || s === undefined) return dash;
  const t = Math.round(s);
  if (t >= 3600) return `${Math.floor(t / 3600)}h ${String(Math.floor((t % 3600) / 60)).padStart(2, "0")}m`;
  if (t >= 60) return `${Math.floor(t / 60)}m ${String(t % 60).padStart(2, "0")}s`;
  return s < 10 && s > 0 ? `${s.toFixed(1)}s` : `${t}s`;
}

export function ms(n: number | null | undefined): string {
  if (n === null || n === undefined) return "n/a";
  return n >= 1000 ? `${(n / 1000).toFixed(1)}s` : `${Math.round(n)}ms`;
}

export function clock(iso: string): string {
  return new Date(iso).toLocaleTimeString("en-GB", { hour12: false });
}

export function dateTime(iso: string): string {
  const d = new Date(iso);
  return `${d.toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })} ${d.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" })}`;
}

export function shortDate(iso: string): string {
  return new Date(iso).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export function signedPct(change: number | null): string {
  if (change === null) return dash;
  return `${change >= 0 ? "+" : ""}${change.toFixed(1)}%`;
}

export function percentChange(next: number, prev: number): number | null {
  return prev === 0 ? null : ((next - prev) / prev) * 100;
}

export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

export function modelKey(providerId: string, modelId: string): string {
  return `${providerId}/${modelId}`;
}
