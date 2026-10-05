import { useEffect, useMemo, useState } from "react";
import type { RunResult } from "@pulsebench/types";
import { Badge, Bar, Button, Empty, PageHeader, Panel, Spinner, Tabs } from "../components/ui";
import { ShareCardModal } from "../components/ShareCard";
import { api } from "../ipc/commands";
import { clock, cx, dateTime, fixed, mb, pct, secs, thousands, dash } from "../lib/format";
import { saveExport } from "../lib/exporting";
import { lowerIsBetter, rankModels, sortLabels, sortModels, sortValue, summarySentence, type SortKey } from "../lib/results";
import { useApp } from "../store/app";
import { useNav } from "../store/nav";

export function useRun(runId: string | undefined) {
  const [run, setRun] = useState<RunResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!runId) return;
    setRun(null);
    setError(null);
    api.getRun(runId).then(setRun).catch((e) => setError(String(e)));
  }, [runId]);
  return { run, error };
}

const columns: Array<{ key: SortKey; label: string; align?: "left" | "right" }> = [
  { key: "score", label: "Score", align: "right" },
  { key: "passRate", label: "Pass", align: "right" },
  { key: "correctness", label: "Correct", align: "right" },
  { key: "tokens", label: "Tok/s", align: "right" },
  { key: "latency", label: "Avg gen", align: "right" },
  { key: "vram", label: "VRAM", align: "right" },
  { key: "memory", label: "RAM", align: "right" },
  { key: "runtime", label: "Time", align: "right" },
];

export function Results() {
  const route = useNav((s) => s.route);
  const go = useNav((s) => s.go);
  const runs = useApp((s) => s.runs);
  const notify = useApp((s) => s.notify);
  const runId = route.page === "results" && route.runId ? route.runId : runs.find((r) => r.status !== "running")?.id;
  const { run, error } = useRun(runId);
  const [sort, setSort] = useState<{ key: SortKey; desc: boolean } | null>(null);
  const [checked, setChecked] = useState<string[]>([]);
  const [tab, setTab] = useState<"board" | "log">("board");
  const [card, setCard] = useState<string | null>(null);

  const ranked = useMemo(() => (run ? rankModels(run.models) : []), [run]);
  const rows = useMemo(() => (run && sort ? sortModels(run.models, sort.key, sort.desc) : ranked), [run, sort, ranked]);

  if (runs.length === 0) {
    return <Empty title="No results yet" action={<Button variant="primary" onClick={() => go({ page: "new" })}>Run a benchmark</Button>}>Finished runs are stored locally and listed here.</Empty>;
  }
  if (error) return <p className="text-bad">{error}</p>;
  if (!run) return <Spinner label="Loading run…" />;

  const gpu = run.system.gpus[0];
  const sentence = summarySentence(run, ranked);
  const best = new Map<SortKey, number>();
  for (const c of columns) {
    const vals = run.models.map((m) => sortValue(m, c.key)).filter((v): v is number => v !== null);
    if (vals.length > 1) best.set(c.key, lowerIsBetter.has(c.key) ? Math.min(...vals) : Math.max(...vals));
  }

  async function doExport(fmt: "json" | "markdown") {
    try {
      const where = await saveExport(run!.id, fmt);
      if (where) notify("ok", `Saved ${where}`);
    } catch (e) {
      notify("error", String(e));
    }
  }

  function headerClick(key: SortKey) {
    setSort((s) => (s?.key === key ? { key, desc: !s.desc } : { key, desc: !lowerIsBetter.has(key) }));
  }

  const winner = ranked[0];
  return (
    <div className="mx-auto max-w-[1180px]">
      <PageHeader
        title={`${run.suite.name.toUpperCase()} — ${gpu?.name ?? "CPU only"}`}
        sub={
          <span className="flex flex-wrap items-center gap-2">
            <span>{dateTime(run.createdAt)}</span><span className="text-dim">·</span><span className="num">suite v{run.suite.version}</span>
            {run.suite.official ? <Badge kind="accent">official suite</Badge> : <Badge kind="warn">custom / modified suite</Badge>}
            {run.standardSettings ? <Badge kind="ok">standard settings</Badge> : <Badge kind="warn">non-standard settings</Badge>}
            {run.status !== "completed" && <Badge kind={run.status === "failed" ? "bad" : "warn"}>{run.status}</Badge>}
          </span>
        }
        actions={
          <>
            <select aria-label="Select run" value={run.id} onChange={(e) => { setChecked([]); setSort(null); go({ page: "results", runId: e.target.value }); }} className="rounded-sm border border-line2 bg-panel2 px-2 py-1.5 text-[12px]">
              {runs.map((r) => <option key={r.id} value={r.id}>{dateTime(r.createdAt)} · {r.suiteName}</option>)}
            </select>
            <Button onClick={() => void doExport("json")}>Export JSON</Button>
            <Button onClick={() => void doExport("markdown")}>Export Markdown</Button>
          </>
        }
      />

      {sentence && <p className="mb-4 rounded-md border border-line bg-panel px-4 py-3 text-[13px] leading-relaxed text-text/90">{sentence}</p>}

      <Tabs tabs={[{ id: "board", label: "Leaderboard" }, { id: "log", label: `Log (${run.log.length})` }]} value={tab} onChange={setTab} />

      {tab === "board" ? (
        <div className="mt-4">
          <Panel pad={false}>
            <table className="w-full border-collapse text-[12.5px]">
              <thead>
                <tr className="border-b border-line text-left">
                  <th className="w-8 px-3 py-2" />
                  <th className="label px-2 py-2">Rank</th>
                  <th className="label px-2 py-2">Model</th>
                  {columns.map((c) => (
                    <th key={c.key} className="label px-2 py-2 text-right">
                      <button onClick={() => headerClick(c.key)} className={cx("uppercase tracking-[0.12em] hover:text-text", sort?.key === c.key && "text-accent2")} title={`Sort by ${sortLabels[c.key]}`}>
                        {c.label}{sort?.key === c.key ? (sort.desc ? " ↓" : " ↑") : ""}
                      </button>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((m) => {
                  const rank = ranked.indexOf(m) + 1;
                  const vram = m.stats.resources.peakVramMb;
                  const cell = (key: SortKey, text: string) => {
                    const v = sortValue(m, key);
                    return <td key={key} className={cx("num px-2 py-2.5 text-right", v !== null && best.get(key) === v ? "font-semibold text-accent2" : "text-text/90", v === null && "text-dim")}>{text}</td>;
                  };
                  return (
                    <tr key={m.key} className="border-b border-line/60 last:border-0 hover:bg-panel2">
                      <td className="px-3 py-2.5"><input type="checkbox" aria-label={`Compare ${m.model.displayName}`} checked={checked.includes(m.key)} onChange={() => setChecked((c) => (c.includes(m.key) ? c.filter((k) => k !== m.key) : [...c, m.key]))} className="accent-[var(--color-accent)]" /></td>
                      <td className="num px-2 py-2.5 text-muted">{m.score ? rank : dash}</td>
                      <td className="px-2 py-2.5">
                        <button className="text-left font-medium hover:text-accent2" onClick={() => go({ page: "model", runId: run.id, modelKey: m.key })}>{m.model.displayName}</button>
                        <div className="text-[11px] text-dim">{m.model.parameterSize ?? ""} {m.model.quantization ?? ""} · {m.model.providerId}</div>
                        {m.status === "failed" && <div className="text-[11px] text-bad">{m.error ?? "did not finish"}</div>}
                        {m.status === "cancelled" && <div className="text-[11px] text-warn">cancelled — partial results</div>}
                      </td>
                      {cell("score", m.score ? thousands(m.score.pulsebenchScore) : "DNF")}
                      {cell("passRate", pct(m.stats.tasksTotal - m.stats.tasksSkipped > 0 ? m.stats.passRate : null))}
                      {cell("correctness", pct(m.score?.components.correctness ?? null))}
                      {cell("tokens", fixed(m.stats.avgTokensPerSecond, 1))}
                      {cell("latency", m.stats.avgGenerationSeconds !== null ? `${m.stats.avgGenerationSeconds.toFixed(1)}s` : "n/a")}
                      {cell("vram", mb(vram))}
                      {cell("memory", mb(m.stats.resources.peakRamMb))}
                      {cell("runtime", secs(m.stats.totalSeconds))}
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </Panel>

          <div className="mt-3 flex flex-wrap items-center gap-2">
            <Button disabled={checked.length < 2} onClick={() => go({ page: "compare", runId: run.id, modelKeys: checked })}>Compare selected ({checked.length})</Button>
            {winner?.score && <Button variant="ghost" onClick={() => setCard(winner.key)}>Share card for {winner.model.displayName}</Button>}
            <span className="text-[11.5px] text-dim">Click a column to sort. Highlighted values are the best in each column. “n/a” and “unavailable” mean the metric could not be measured.</span>
          </div>

          <div className="mt-4 grid gap-4 md:grid-cols-2">
            {run.notes.length > 0 && (
              <Panel title="Notes about this run">
                <ul className="list-disc space-y-1 pl-4 text-[12px] text-muted">{run.notes.map((n, i) => <li key={i}>{n}</li>)}</ul>
              </Panel>
            )}
            <Panel title="Reproducibility">
              <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-[12px]">
                <dt className="text-dim">Run ID</dt><dd className="num truncate">{run.id}</dd>
                <dt className="text-dim">PulseBench</dt><dd className="num">{run.appVersion} · {run.schema}</dd>
                <dt className="text-dim">Suite hash</dt><dd className="num truncate" title={run.suite.contentHash}>{run.suite.contentHash.slice(0, 16)}…</dd>
                <dt className="text-dim">Generation</dt><dd className="num">T={run.settings.generation.temperature} seed={run.settings.generation.seed ?? "none"} ctx={run.settings.generation.contextTokens} max={run.settings.generation.maxOutputTokens}</dd>
                <dt className="text-dim">Providers</dt><dd className="num">{run.providers.map((p) => `${p.name}${p.version ? ` ${p.version}` : ""}`).join(", ")}</dd>
                <dt className="text-dim">Score formula</dt><dd className="num">{run.settings.scoring.formula}</dd>
              </dl>
              <div className="mt-3"><Bar value={run.models.length ? ranked.filter((m) => m.status === "completed").length / run.models.length : 0} tone="ok" /></div>
              <div className="mt-1 text-[11px] text-dim">{ranked.filter((m) => m.status === "completed").length}/{run.models.length} models completed</div>
            </Panel>
          </div>
        </div>
      ) : (
        <Panel className="mt-4" title="Run log">
          <ul className="max-h-[560px] space-y-0.5 overflow-y-auto">
            {run.log.map((e, i) => (
              <li key={i} className={cx("num flex gap-3 text-[11.5px]", e.level === "error" ? "text-bad" : e.level === "warn" ? "text-warn" : "text-muted")}>
                <span className="shrink-0 text-dim">{clock(e.ts)}</span><span className="w-20 shrink-0 text-accent/80">{e.scope}</span><span className="whitespace-pre-wrap break-words">{e.message}</span>
              </li>
            ))}
          </ul>
        </Panel>
      )}
      {card && <ShareCardModal runId={run.id} modelKey={card} onClose={() => setCard(null)} />}
    </div>
  );
}
