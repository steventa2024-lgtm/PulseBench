import { useEffect, useMemo, useState } from "react";
import type { HistoryPoint } from "@pulsebench/types";
import { Badge, Button, Empty, PageHeader, Panel } from "../components/ui";
import { api } from "../ipc/commands";
import { saveExport } from "../lib/exporting";
import { cx, dateTime, dash, percentChange, shortDate, signedPct, thousands } from "../lib/format";
import { useApp } from "../store/app";
import { useNav } from "../store/nav";

export interface Delta { change: number | null; comparable: boolean }

/** Change between consecutive points; only comparable when suite id, version and settings class match. */
export function deltas(points: HistoryPoint[]): Delta[] {
  return points.map((p, i) => {
    const prev = points[i - 1];
    if (!prev || p.score === null || prev.score === null) return { change: null, comparable: true };
    const comparable = prev.suiteId === p.suiteId && prev.suiteVersion === p.suiteVersion && prev.standardSettings === p.standardSettings;
    return { change: percentChange(p.score, prev.score), comparable };
  });
}

function Chart({ points }: { points: HistoryPoint[] }) {
  const scored = points.filter((p) => p.score !== null);
  if (scored.length < 2) return null;
  const w = 640, h = 140, pad = 24;
  const vals = scored.map((p) => p.score as number);
  const lo = Math.min(...vals), hi = Math.max(...vals), span = Math.max(hi - lo, 1);
  const x = (i: number) => pad + (i / (scored.length - 1)) * (w - pad * 2);
  const y = (v: number) => h - pad - ((v - lo) / span) * (h - pad * 2);
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="w-full max-w-[640px]" role="img" aria-label="Score history">
      <polyline fill="none" stroke="var(--color-accent)" strokeWidth="1.5" points={scored.map((p, i) => `${x(i)},${y(p.score as number)}`).join(" ")} />
      {scored.map((p, i) => (
        <g key={p.runId}>
          <circle cx={x(i)} cy={y(p.score as number)} r="3" fill="var(--color-accent2)" />
          <text x={x(i)} y={y(p.score as number) - 8} textAnchor="middle" fontSize="10" fill="var(--color-muted)" fontFamily="var(--font-mono)">{thousands(p.score as number)}</text>
          <text x={x(i)} y={h - 6} textAnchor="middle" fontSize="9.5" fill="var(--color-dim)">{shortDate(p.createdAt)}</text>
        </g>
      ))}
    </svg>
  );
}

export function History() {
  const { runs, latest, refreshRuns, notify } = useApp();
  const go = useNav((s) => s.go);
  const [modelKey, setModelKey] = useState<string | null>(null);
  const [points, setPoints] = useState<HistoryPoint[]>([]);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  const modelKeys = useMemo(() => [...new Set(runs.flatMap((r) => r.models.map((m) => m.key)))], [runs]);
  const names = useMemo(() => new Map(runs.flatMap((r) => r.models.map((m) => [m.key, m.displayName] as const))), [runs]);
  useEffect(() => {
    if (!modelKey && modelKeys[0]) setModelKey(latest[0]?.modelKey ?? modelKeys[0]);
  }, [modelKey, modelKeys, latest]);
  useEffect(() => {
    if (modelKey) void api.modelHistory(modelKey).then(setPoints).catch((e) => notify("error", String(e)));
  }, [modelKey, runs, notify]);

  if (runs.length === 0) return <Empty title="No history yet">Completed and cancelled runs are saved here and stay available after you close PulseBench.</Empty>;
  const d = deltas(points);

  async function remove(id: string) {
    try {
      await api.deleteRun(id);
      setConfirmDelete(null);
      await refreshRuns();
    } catch (e) {
      notify("error", String(e));
    }
  }

  return (
    <div className="mx-auto max-w-[1100px]">
      <PageHeader title="History" sub="Every run is stored locally. Reopen, export or compare over time." />

      <Panel title="Score over time" actions={
        <select aria-label="Model" value={modelKey ?? ""} onChange={(e) => setModelKey(e.target.value)} className="rounded-sm border border-line2 bg-panel2 px-2 py-1 text-[12px]">
          {modelKeys.map((k) => <option key={k} value={k}>{names.get(k) ?? k}</option>)}
        </select>
      }>
        {points.length === 0 ? <p className="text-muted">No scored runs for this model yet.</p> : (
          <>
            <Chart points={points} />
            <table className="mt-3 w-full text-[12.5px]">
              <thead><tr className="text-left"><th className="label py-1">Date</th><th className="label py-1">Suite</th><th className="label py-1 text-right">Score</th><th className="label py-1 text-right">Change</th><th className="label py-1 pl-4">Hardware</th></tr></thead>
              <tbody>
                {[...points].map((p, i) => ({ p, d: d[i]! })).reverse().map(({ p, d }) => (
                  <tr key={p.runId} className="cursor-pointer border-t border-line/60 hover:bg-panel2" onClick={() => go({ page: "results", runId: p.runId })}>
                    <td className="py-1.5">{dateTime(p.createdAt)}</td>
                    <td className="py-1.5 text-muted">{p.suiteId} v{p.suiteVersion} {!p.standardSettings && <Badge kind="warn">custom settings</Badge>}</td>
                    <td className="num py-1.5 text-right text-accent2">{p.score !== null ? thousands(p.score) : dash}</td>
                    <td className={cx("num py-1.5 text-right", d.change === null ? "text-dim" : d.comparable ? (d.change >= 0 ? "text-ok" : "text-bad") : "text-dim")} title={d.comparable ? undefined : "Different suite version or settings: not directly comparable"}>
                      {signedPct(d.change)}{d.change !== null && !d.comparable && " ⚠"}
                    </td>
                    <td className="py-1.5 pl-4 text-[11.5px] text-muted">{p.gpu ?? "no GPU"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="mt-2 text-[11px] text-dim">Changes marked ⚠ compare runs of different suite versions or settings and should not be read as regressions or improvements.</p>
          </>
        )}
      </Panel>

      <h2 className="label mb-2 mt-6">All runs</h2>
      <Panel pad={false}>
        <ul className="divide-y divide-line">
          {runs.map((r) => (
            <li key={r.id} className="flex items-center gap-3 px-3.5 py-2.5">
              <button className="min-w-0 flex-1 text-left" onClick={() => go({ page: "results", runId: r.id })}>
                <div className="font-medium">{r.suiteName} <span className="text-dim">v{r.suiteVersion}</span> {r.status !== "completed" && <Badge kind={r.status === "failed" ? "bad" : "warn"}>{r.status}</Badge>}</div>
                <div className="truncate text-[11.5px] text-muted">{dateTime(r.createdAt)} · {r.gpu ?? r.cpu} · {r.models.map((m) => `${m.displayName} ${m.score !== null ? thousands(m.score) : "DNF"}`).join(" · ")}</div>
              </button>
              <Button variant="ghost" onClick={() => void saveExport(r.id, "json").catch((e) => notify("error", String(e)))}>JSON</Button>
              <Button variant="ghost" onClick={() => void saveExport(r.id, "markdown").catch((e) => notify("error", String(e)))}>Markdown</Button>
              {confirmDelete === r.id ? (
                <><Button variant="danger" onClick={() => void remove(r.id)}>Delete run</Button><Button variant="ghost" onClick={() => setConfirmDelete(null)}>Keep</Button></>
              ) : <Button variant="ghost" onClick={() => setConfirmDelete(r.id)}>Delete</Button>}
            </li>
          ))}
        </ul>
      </Panel>
    </div>
  );
}
