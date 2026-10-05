import { useMemo, useState } from "react";
import type { ModelResult } from "@pulsebench/types";
import { Bar, Empty, PageHeader, Panel, Spinner } from "../components/ui";
import { cx, dash, fixed, mb, ms, pct, percentChange, secs, signedPct, thousands, dateTime } from "../lib/format";
import { componentRows, rankModels } from "../lib/results";
import { useApp } from "../store/app";
import { useNav } from "../store/nav";
import { useRun } from "./Results";

type Row = { label: string; get: (m: ModelResult) => number | null; fmt: (v: number | null) => string; lowerBetter?: boolean };

const categories = ["bugfix", "implementation", "refactoring", "testing", "security", "reasoning"] as const;
const catLabel = { bugfix: "Bug fixing", implementation: "Implementation", refactoring: "Refactoring", testing: "Testing", security: "Security", reasoning: "Reasoning" };

const rows: Row[] = [
  { label: "PulseBench Score", get: (m) => m.score?.pulsebenchScore ?? null, fmt: (v) => (v === null ? "DNF" : thousands(v)) },
  { label: "Pass rate", get: (m) => (m.stats.tasksTotal - m.stats.tasksSkipped > 0 ? m.stats.passRate : null), fmt: (v) => pct(v) },
  { label: "Tokens / sec", get: (m) => m.stats.avgTokensPerSecond, fmt: (v) => fixed(v, 1) },
  { label: "Avg latency (first token)", get: (m) => m.stats.avgTimeToFirstTokenMs, fmt: ms, lowerBetter: true },
  { label: "Avg generation time", get: (m) => m.stats.avgGenerationSeconds, fmt: (v) => (v === null ? "n/a" : `${v.toFixed(1)}s`), lowerBetter: true },
  { label: "Peak VRAM", get: (m) => m.stats.resources.peakVramMb, fmt: (v) => mb(v), lowerBetter: true },
  { label: "Peak system RAM", get: (m) => m.stats.resources.peakRamMb, fmt: (v) => mb(v), lowerBetter: true },
  { label: "Total time", get: (m) => m.stats.totalSeconds, fmt: (v) => secs(v), lowerBetter: true },
  ...categories.map((c): Row => ({ label: catLabel[c], get: (m) => m.score?.categories.find((x) => x.category === c)?.score ?? null, fmt: (v) => (v === null ? dash : String(Math.round(v))) })),
  ...componentRows.map((r): Row => ({ label: `${r.label} (component)`, get: (m) => m.score?.components[r.key] ?? null, fmt: (v) => pct(v) })),
];

export function Compare() {
  const route = useNav((s) => s.route);
  const runs = useApp((s) => s.runs);
  const go = useNav((s) => s.go);
  const runId = route.page === "compare" && route.runId ? route.runId : runs.find((r) => r.models.length > 1)?.id ?? runs[0]?.id;
  const { run, error } = useRun(runId);
  const initial = route.page === "compare" ? route.modelKeys : undefined;
  const [picked, setPicked] = useState<string[] | undefined>(initial);

  const ranked = useMemo(() => (run ? rankModels(run.models) : []), [run]);
  if (runs.length === 0) return <Empty title="Nothing to compare yet">Run a benchmark with two or more models first.</Empty>;
  if (error) return <p className="text-bad">{error}</p>;
  if (!run) return <Spinner label="Loading…" />;
  const keys = picked && picked.length >= 2 ? picked : ranked.slice(0, 2).map((m) => m.key);
  const cols = keys.map((k) => run.models.find((m) => m.key === k)).filter((m): m is ModelResult => !!m);

  return (
    <div className="mx-auto max-w-[1100px]">
      <PageHeader
        title="Compare models"
        sub={`${run.suite.name} v${run.suite.version} · ${dateTime(run.createdAt)} · every model ran identical tasks`}
        actions={
          <select aria-label="Select run" value={run.id} onChange={(e) => { setPicked(undefined); go({ page: "compare", runId: e.target.value }); }} className="rounded-sm border border-line2 bg-panel2 px-2 py-1.5 text-[12px]">
            {runs.map((r) => <option key={r.id} value={r.id}>{dateTime(r.createdAt)} · {r.suiteName} · {r.models.length} models</option>)}
          </select>
        }
      />
      <div className="mb-4 flex flex-wrap gap-2">
        {run.models.map((m) => {
          const on = keys.includes(m.key);
          return (
            <button key={m.key} onClick={() => setPicked(on ? keys.filter((k) => k !== m.key) : [...keys, m.key])} className={cx("rounded-sm border px-2.5 py-1 text-[12px]", on ? "border-accent/60 bg-accent/10 text-text" : "border-line text-muted hover:border-line2")}>
              {m.model.displayName}
            </button>
          );
        })}
      </div>
      {cols.length < 2 ? <Empty title="Select at least two models" /> : (
        <>
          <div className="grid gap-4 md:grid-cols-2">
            {[{ title: "PulseBench Score", get: (m: ModelResult) => m.score?.pulsebenchScore ?? null, max: 10000 }, { title: "Pass rate", get: (m: ModelResult) => m.stats.passRate, max: 1 }].map((chart) => (
              <Panel key={chart.title} title={chart.title}>
                <ul className="space-y-2.5">
                  {cols.map((m) => {
                    const v = chart.get(m);
                    return (
                      <li key={m.key} className="grid grid-cols-[130px_1fr_64px] items-center gap-3 text-[12px]">
                        <span className="truncate text-muted" title={m.model.displayName}>{m.model.displayName}</span>
                        <Bar value={v} max={chart.max} height={8} />
                        <span className="num text-right">{chart.max === 1 ? pct(v) : v === null ? "DNF" : thousands(v)}</span>
                      </li>
                    );
                  })}
                </ul>
              </Panel>
            ))}
          </div>
          <Panel className="mt-4" pad={false}>
            <table className="w-full text-[12.5px]">
              <thead>
                <tr className="border-b border-line">
                  <th className="label px-3 py-2 text-left" />
                  {cols.map((m) => <th key={m.key} className="px-3 py-2 text-right text-[12px] font-semibold">{m.model.displayName}</th>)}
                </tr>
              </thead>
              <tbody>
                {rows.map((r) => {
                  const vals = cols.map((m) => r.get(m));
                  const present = vals.filter((v): v is number => v !== null);
                  const best = present.length > 1 ? (r.lowerBetter ? Math.min(...present) : Math.max(...present)) : null;
                  const base = vals[0] ?? null;
                  return (
                    <tr key={r.label} className="border-b border-line/50 last:border-0">
                      <td className="px-3 py-2 text-muted">{r.label}</td>
                      {vals.map((v, i) => (
                        <td key={cols[i]!.key} className={cx("num px-3 py-2 text-right", v !== null && v === best && "font-semibold text-accent2", v === null && "text-dim")}>
                          {r.fmt(v)}
                          {i > 0 && v !== null && base !== null && <span className="ml-2 text-[10.5px] text-dim">{signedPct(percentChange(v, base))}</span>}
                        </td>
                      ))}
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </Panel>
          <p className="mt-2 text-[11px] text-dim">Percentages show the difference relative to the first column. Highlighted = best in row (lower is better for time and memory).</p>
        </>
      )}
    </div>
  );
}
