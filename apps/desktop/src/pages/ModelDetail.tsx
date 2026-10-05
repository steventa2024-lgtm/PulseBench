import { useState } from "react";
import type { ModelResult, TelemetrySeries } from "@pulsebench/types";
import { Badge, Bar, Button, PageHeader, Panel, Sparkline, Spinner, Stat } from "../components/ui";
import { ShareCardModal } from "../components/ShareCard";
import { TaskDrawer } from "../components/TaskDrawer";
import { ModelMeta } from "../components/model";
import { dash, fixed, mb, pct, secs, thousands } from "../lib/format";
import { componentRows } from "../lib/results";
import { useNav } from "../store/nav";
import { useRun } from "./Results";

const categoryOrder = ["bugfix", "implementation", "refactoring", "testing", "security", "reasoning"] as const;
const categoryLabel: Record<(typeof categoryOrder)[number], string> = { bugfix: "Bug fixing", implementation: "Implementation", refactoring: "Refactoring", testing: "Testing", security: "Security", reasoning: "Reasoning" };

function tone(v: number): "ok" | "warn" | "bad" { return v >= 80 ? "ok" : v >= 50 ? "warn" : "bad"; }

function Telemetry({ series }: { series: TelemetrySeries | undefined }) {
  const s = series?.samples ?? [];
  if (s.length < 2) return <p className="text-muted">No telemetry samples were recorded.</p>;
  const hasGpu = s.some((x) => x.gpuPercent !== null);
  return (
    <div className="flex flex-wrap gap-8 text-[11px] text-dim">
      <div>GPU utilization {hasGpu ? <Sparkline values={s.map((x) => x.gpuPercent)} max={100} width={220} /> : <div className="mt-1 text-muted">unavailable (no nvidia-smi)</div>}</div>
      <div>VRAM used {hasGpu ? <Sparkline values={s.map((x) => x.vramUsedMb)} width={220} tone="accent" /> : <div className="mt-1 text-muted">unavailable</div>}</div>
      <div>CPU <Sparkline values={s.map((x) => x.cpuPercent)} max={100} width={220} tone="ok" /></div>
      <div>System RAM <Sparkline values={s.map((x) => x.ramUsedMb)} width={220} tone="accent" /></div>
    </div>
  );
}

function Breakdown({ m, weights }: { m: ModelResult; weights: Record<string, number> }) {
  const score = m.score;
  if (!score) return null;
  const applicable = componentRows.filter((r) => score.components[r.key] !== null);
  const totalW = applicable.reduce((n, r) => n + (weights[r.weightKey] ?? 0), 0);
  return (
    <table className="w-full text-[12px]">
      <thead><tr className="text-left"><th className="label py-1">Component</th><th className="label py-1 text-right">Weight</th><th className="label py-1 text-right">Value</th><th className="label py-1 text-right">Points</th></tr></thead>
      <tbody>
        {componentRows.map((r) => {
          const v = score.components[r.key];
          const w = weights[r.weightKey] ?? 0;
          return (
            <tr key={r.key} className="border-t border-line/60" title={r.help}>
              <td className="py-1.5">{r.label}</td>
              <td className="num py-1.5 text-right text-muted">{v === null ? dash : `${((w / totalW) * 100).toFixed(1)}%`}</td>
              <td className="num py-1.5 text-right">{v === null ? <span className="text-dim">n/a</span> : pct(v)}</td>
              <td className="num py-1.5 text-right text-accent2">{v === null ? dash : thousands((v * w) / totalW * 10000)}</td>
            </tr>
          );
        })}
        <tr className="border-t border-line2"><td className="py-1.5 font-semibold" colSpan={3}>PulseBench Score</td><td className="num py-1.5 text-right font-semibold text-accent2">{thousands(score.pulsebenchScore)}</td></tr>
      </tbody>
    </table>
  );
}

export function ModelDetail({ runId, modelKey, taskId }: { runId: string; modelKey: string; taskId?: string }) {
  const go = useNav((s) => s.go);
  const { run, error } = useRun(runId);
  const [openTask, setOpenTask] = useState<string | undefined>(taskId);
  const [card, setCard] = useState(false);
  if (error) return <p className="text-bad">{error}</p>;
  if (!run) return <Spinner label="Loading…" />;
  const m = run.models.find((x) => x.key === modelKey);
  if (!m) return <p className="text-bad">Model not found in this run.</p>;
  const s = m.stats;
  const scored = s.tasksTotal - s.tasksSkipped;
  const peak = s.resources;
  const task = m.tasks.find((t) => t.id === openTask);
  const weights = run.settings.scoring.weights as unknown as Record<string, number>;

  return (
    <div className="mx-auto max-w-[1180px]">
      <PageHeader
        title={m.model.displayName}
        sub={<span className="flex items-center gap-2"><ModelMeta m={m.model} /><Badge>{m.model.providerId}</Badge>{m.status !== "completed" && <Badge kind={m.status === "failed" ? "bad" : "warn"}>{m.status}</Badge>}</span>}
        actions={<><Button onClick={() => go({ page: "results", runId })}>← Leaderboard</Button>{m.score && <Button variant="primary" onClick={() => setCard(true)}>Share card</Button>}</>}
      />
      {m.error && <p className="mb-4 rounded-sm border border-bad/40 bg-bad/10 p-3 text-bad">{m.error}</p>}

      <Panel>
        <div className="grid grid-cols-2 gap-x-6 gap-y-4 md:grid-cols-3 xl:grid-cols-6">
          <Stat label="PulseBench Score" value={m.score ? thousands(m.score.pulsebenchScore) : "DNF"} tone="accent" />
          <Stat label="Tests passed" value={pct(scored > 0 ? s.passRate : null)} />
          <Stat label="Tasks completed" value={`${s.tasksPassed} / ${scored}`} sub={s.tasksSkipped ? `${s.tasksSkipped} skipped` : undefined} />
          <Stat label="Avg generation" value={s.avgGenerationSeconds !== null ? `${s.avgGenerationSeconds.toFixed(1)} sec` : "n/a"} />
          <Stat label="Avg tokens/s" value={fixed(s.avgTokensPerSecond, 1)} />
          <Stat label="Peak VRAM" value={mb(peak.peakVramMb)} sub={m.residency?.sizeVramBytes != null ? `model resident: ${(m.residency.sizeVramBytes / 1e9).toFixed(1)} GB (provider)` : undefined} />
        </div>
      </Panel>

      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        <Panel title="Category breakdown (0–100)">
          <ul className="space-y-2.5">
            {categoryOrder.map((c) => {
              const cs = m.score?.categories.find((x) => x.category === c);
              return (
                <li key={c} className="grid grid-cols-[120px_1fr_44px] items-center gap-3 text-[12.5px]">
                  <span className="text-muted">{categoryLabel[c]}</span>
                  <Bar value={cs ? cs.score : null} max={100} tone={cs ? tone(cs.score) : "accent"} height={6} />
                  <span className="num text-right">{cs ? Math.round(cs.score) : <span className="text-dim">—</span>}</span>
                </li>
              );
            })}
          </ul>
          <p className="mt-3 text-[11px] text-dim">A “—” means the suite has no task in that category.</p>
        </Panel>
        <Panel title="How this score was calculated">
          <Breakdown m={m} weights={weights} />
          <p className="mt-2 text-[11px] text-dim">Components are difficulty-weighted means over tasks where they apply; weights of components that did not apply are renormalized. {run.settings.scoring.formula}</p>
        </Panel>
      </div>

      <Panel className="mt-4" title="Failures & reliability">
        <div className="grid grid-cols-2 gap-4 md:grid-cols-5">
          <Stat label="Repair attempts" value={s.repairAttempts} />
          <Stat label="Compile failures" value={s.compileFailures} />
          <Stat label="Runtime / test failures" value={s.runtimeFailures} />
          <Stat label="Protocol failures" value={s.protocolFailures} />
          <Stat label="Total time" value={secs(s.totalSeconds)} sub={`${thousands(s.totalCompletionTokens)} tokens generated`} />
        </div>
      </Panel>

      <Panel className="mt-4" title="Resource usage during this model's run" actions={<span className="num text-[11px] text-dim">{peak.samples} samples · peak GPU {peak.peakGpuPercent != null ? `${peak.peakGpuPercent.toFixed(0)}%` : "unavailable"} · peak CPU {peak.peakCpuPercent != null ? `${peak.peakCpuPercent.toFixed(0)}%` : "unavailable"} · peak RAM {mb(peak.peakRamMb)}</span>}>
        <Telemetry series={run.telemetry.find((t) => t.modelKey === m.key)} />
      </Panel>

      <h2 className="label mb-2 mt-6">Tasks</h2>
      <Panel pad={false}>
        <table className="w-full text-[12.5px]">
          <thead><tr className="border-b border-line text-left"><th className="label px-3 py-2">Task</th><th className="label px-2 py-2">Category</th><th className="label px-2 py-2">Result</th><th className="label px-2 py-2 text-right">Score</th><th className="label px-2 py-2 text-right">Attempts</th><th className="label px-2 py-2 text-right">Tok/s</th><th className="label px-3 py-2 text-right">Time</th></tr></thead>
          <tbody>
            {m.tasks.map((t) => {
              const g = t.attempts.at(-1)?.generation;
              return (
                <tr key={t.id} className="cursor-pointer border-b border-line/60 last:border-0 hover:bg-panel2" onClick={() => setOpenTask(t.id)}>
                  <td className="px-3 py-2"><div className="font-medium">{t.title}</div><div className="num text-[11px] text-dim">{t.id} · {t.language} · {t.difficulty}</div></td>
                  <td className="px-2 py-2 text-muted">{categoryLabel[t.category]}</td>
                  <td className="px-2 py-2"><Badge kind={t.status === "passed" ? "ok" : t.status === "failed" ? "bad" : "warn"}>{t.status}</Badge></td>
                  <td className="num px-2 py-2 text-right text-accent2">{t.score ? t.score.value.toFixed(1) : dash}</td>
                  <td className="num px-2 py-2 text-right text-muted">{t.attempts.length}/{t.maxAttempts}</td>
                  <td className="num px-2 py-2 text-right text-muted">{fixed(g?.tokensPerSecond ?? null, 1)}</td>
                  <td className="num px-3 py-2 text-right text-muted">{secs(t.durationMs / 1000)}</td>
                </tr>
              );
            })}
            {m.tasks.length === 0 && <tr><td className="px-3 py-4 text-muted" colSpan={7}>No tasks ran for this model.</td></tr>}
          </tbody>
        </table>
      </Panel>
      {task && <TaskDrawer key={task.id} task={task} onClose={() => setOpenTask(undefined)} />}
      {card && <ShareCardModal runId={runId} modelKey={modelKey} onClose={() => setCard(false)} />}
    </div>
  );
}
