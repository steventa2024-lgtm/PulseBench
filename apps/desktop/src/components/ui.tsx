import { Component, type ErrorInfo, type ReactNode } from "react";
import { cx } from "../lib/format";

export function Panel({ title, actions, children, className, pad = true }: { title?: ReactNode; actions?: ReactNode; children: ReactNode; className?: string; pad?: boolean }) {
  return (
    <section className={cx("border border-line bg-panel rounded-md", className)}>
      {(title || actions) && (
        <header className="flex items-center justify-between gap-3 border-b border-line px-3.5 py-2">
          <h2 className="label">{title}</h2>
          <div className="flex items-center gap-2">{actions}</div>
        </header>
      )}
      <div className={pad ? "p-3.5" : ""}>{children}</div>
    </section>
  );
}

export function Stat({ label, value, sub, tone }: { label: string; value: ReactNode; sub?: ReactNode; tone?: "ok" | "bad" | "accent" | "muted" }) {
  const color = tone === "ok" ? "text-ok" : tone === "bad" ? "text-bad" : tone === "accent" ? "text-accent2" : tone === "muted" ? "text-muted" : "text-text";
  return (
    <div className="min-w-0">
      <div className="label">{label}</div>
      <div className={cx("num mt-0.5 truncate text-[17px] font-semibold", color)}>{value}</div>
      {sub && <div className="mt-0.5 truncate text-[11px] text-muted">{sub}</div>}
    </div>
  );
}

type BadgeKind = "ok" | "bad" | "warn" | "accent" | "muted";
const badgeStyles: Record<BadgeKind, string> = {
  ok: "border-ok/40 text-ok bg-ok/10",
  bad: "border-bad/40 text-bad bg-bad/10",
  warn: "border-warn/40 text-warn bg-warn/10",
  accent: "border-accent/50 text-accent bg-accent/10",
  muted: "border-line2 text-muted bg-transparent",
};

export function Badge({ kind = "muted", children, title }: { kind?: BadgeKind; children: ReactNode; title?: string }) {
  return (
    <span title={title} className={cx("inline-flex items-center gap-1 rounded-sm border px-1.5 py-px text-[10.5px] font-semibold uppercase tracking-wider", badgeStyles[kind])}>
      {children}
    </span>
  );
}

export function Button({ variant = "default", className, ...rest }: React.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "default" | "primary" | "danger" | "ghost" }) {
  const styles = {
    default: "border-line2 bg-panel2 text-text hover:border-accent/60",
    primary: "border-accent bg-accent text-white hover:bg-accent/85 font-semibold",
    danger: "border-bad/50 bg-bad/10 text-bad hover:bg-bad/20",
    ghost: "border-transparent text-muted hover:text-text hover:bg-panel2",
  }[variant];
  return (
    <button
      {...rest}
      className={cx("inline-flex items-center justify-center gap-1.5 rounded-sm border px-3 py-1.5 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-40", styles, className)}
    />
  );
}

export function Dot({ state }: { state: "ok" | "off" | "bad" | "warn" }) {
  const c = { ok: "bg-ok", off: "bg-dim", bad: "bg-bad", warn: "bg-warn" }[state];
  return <span className={cx("inline-block h-1.5 w-1.5 rounded-full", c, state === "ok" && "shadow-[0_0_6px_var(--color-ok)]")} />;
}

export function Bar({ value, max = 1, tone = "accent", height = 4 }: { value: number | null; max?: number; tone?: "accent" | "ok" | "warn" | "bad"; height?: number }) {
  const w = value === null ? 0 : Math.max(0, Math.min(100, (value / max) * 100));
  const color = { accent: "bg-accent", ok: "bg-ok", warn: "bg-warn", bad: "bg-bad" }[tone];
  return (
    <div className="w-full overflow-hidden rounded-[1px] bg-line" style={{ height }}>
      <div className={cx("h-full transition-[width] duration-300", color)} style={{ width: `${w}%` }} />
    </div>
  );
}

export function Spinner({ label }: { label?: string }) {
  return (
    <span className="inline-flex items-center gap-2 text-muted">
      <span className="h-3 w-3 animate-spin rounded-full border border-line2 border-t-accent2" />
      {label}
    </span>
  );
}

export function Empty({ title, children, action }: { title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="grid-bg flex flex-col items-center justify-center gap-2 rounded-md border border-dashed border-line px-6 py-10 text-center">
      <div className="text-[14px] font-semibold">{title}</div>
      {children && <div className="max-w-md text-muted">{children}</div>}
      {action}
    </div>
  );
}

export function Tabs<T extends string>({ tabs, value, onChange }: { tabs: Array<{ id: T; label: string }>; value: T; onChange: (v: T) => void }) {
  return (
    <div role="tablist" className="flex gap-0 border-b border-line">
      {tabs.map((t) => (
        <button
          key={t.id}
          role="tab"
          aria-selected={value === t.id}
          onClick={() => onChange(t.id)}
          className={cx("-mb-px border-b px-3 py-1.5 text-[12px]", value === t.id ? "border-accent text-text" : "border-transparent text-muted hover:text-text")}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}

export function PageHeader({ title, sub, actions }: { title: string; sub?: ReactNode; actions?: ReactNode }) {
  return (
    <div className="mb-4 flex items-start justify-between gap-4">
      <div>
        <h1 className="text-[18px] font-semibold tracking-tight">{title}</h1>
        {sub && <p className="mt-0.5 text-muted">{sub}</p>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{actions}</div>
    </div>
  );
}

export function Pre({ children, className }: { children: ReactNode; className?: string }) {
  return <pre className={cx("num max-h-[420px] overflow-auto whitespace-pre-wrap break-words rounded-sm border border-line bg-ink p-3 text-[11.5px] leading-relaxed text-text/90", className)}>{children}</pre>;
}

export function Sparkline({ values, width = 160, height = 36, max, tone = "accent2" }: { values: Array<number | null>; width?: number; height?: number; max?: number; tone?: "accent" | "accent2" | "ok" }) {
  const pts = values.filter((v): v is number => v !== null);
  if (pts.length < 2) return <div className="text-[11px] text-dim" style={{ height }}>collecting…</div>;
  const hi = max ?? Math.max(...pts, 1);
  const step = width / (values.length - 1);
  const d = values
    .map((v, i) => (v === null ? null : `${i === 0 || values[i - 1] === null ? "M" : "L"}${(i * step).toFixed(1)},${(height - (v / hi) * (height - 2) - 1).toFixed(1)}`))
    .filter(Boolean)
    .join(" ");
  const stroke = { accent: "var(--color-accent)", accent2: "var(--color-accent2)", ok: "var(--color-ok)" }[tone];
  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="block overflow-visible" aria-hidden>
      <path d={d} fill="none" stroke={stroke} strokeWidth={1.5} strokeLinejoin="round" />
    </svg>
  );
}

interface EBState {
  error: Error | null;
}

export class ErrorBoundary extends Component<{ children: ReactNode; name?: string }, EBState> {
  state: EBState = { error: null };
  static getDerivedStateFromError(error: Error): EBState {
    return { error };
  }
  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("UI error", this.props.name, error, info.componentStack);
  }
  render() {
    if (this.state.error) {
      return (
        <Panel title="Something went wrong in this view">
          <p className="mb-3 text-bad">{this.state.error.message}</p>
          <Button onClick={() => this.setState({ error: null })}>Try again</Button>
        </Panel>
      );
    }
    return this.props.children;
  }
}
