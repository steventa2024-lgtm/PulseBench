import { useEffect, useState } from "react";
import type { LogEntry, TaskStatus } from "@pulsebench/types";
import { Badge, Bar, Button, Empty, Panel, Sparkline, Stat } from "../components/ui";
import { clock, cx, fixed, mb, pct, thousands } from "../lib/format";
import { api } from "../ipc/commands";
import { useApp } from "../store/app";
import { useNav } from "../store/nav";
import { phaseLabel, progressOf } from "../store/live";

const statusColor: Record<TaskStatus, string> = { passed: "bg-ok", failed: "bg-bad", skipped: "bg-dim", error: "bg-warn" };

function LogLine({ e }: { e: LogEntry }) {
  const tone = e.level === "error" ? "text-bad" : e.level === "warn" ? "text-warn" : "text-muted";
  return (
    <li className={cx("num flex gap-3 whitespace-pre-wrap break-words text-[11.5px]", tone)}>
      <span className="shrink-0 text-dim">{clock(e.ts)}</span>
      <span className="w-[84px] shrink-0 text-accent/80">{e.scope}</span>
      <span className="min-w-0">{e.message}</span>
    </li>
  );
}

export function RunLive() {
  const live = useApp((s) => s.live);
  const notify = useApp((s) => s.notify);
  const go = useNav((s) => s.go);
  const [now, setNow] = useState(Date.now());
  const [showAll, setShowAll] = useState(false);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(t);
  }, []);

  if (!live) {
    return (
      <div className="mx-auto max-w-[900px]">
        <Empty title="No benchmark is running" action={<Button variant="primary" onClick={() => go({ page: "new" })}>New benchmark</Button>}>
          Start a run from “New Benchmark”. Progress, GPU usage and the activity log appear here in real time.
        </Empty>
      </div>
    );
  }

  const sample = live.telemetry.at(-1);
  const current = live.models.find((m) => m.key === live.currentModel);
  const progress = progressOf(live);
  const elapsed = live.taskStartedAt ? (now - live.taskStartedAt) / 1000 : 0;
  const g = live.generation;
  const genSecs = g ? Math.max(0.001, (g.elapsedMs - (g.timeToFirstTokenMs ?? 0)) / 1000) : 0;
  const approxRate = g && g.chunks > 1 ? (g.chunks - 1) / genSecs : null;
  const logs = showAll ? live.logs : live.logs.slice(-12);
  const finished = live.finished;

  async function cancel() {
    setConfirming(false);
    try {
      await api.cancelRun();
    } catch (e) {
      notify("error", String(e));
    }
  }

  return (
    <div className="mx-auto max-w-[1000px]">
      <div className="mb-4 flex items-end justify-between">
        <div>
          <div className="label !text-accent">{live.suiteName || "Starting benchmark"}</div>
          <h1 className="mt-0.5 text-[20px] font-semibold tracking-tight">{current?.displayName ?? "Preparing…"}</h1>
          <div className="num text-[11.5px] text-muted">Model {Math.min(live.modelIndex + 1, Math.max(live.modelCount, 1))} of {live.modelCount || "…"}</div>
        </div>
        <div className="flex items-center gap-2">
          {finished ? (
            <>
              <Badge kind={live.status === "completed" ? "ok" : live.status === "cancelled" ? "warn" : "bad"}>{live.status}</Badge>
              <Button variant="primary" onClick={() => go({ page: "results", runId: live.runId })}>View results</Button>
            </>
          ) : confirming ? (
            <>
              <span className="text-muted">Cancel the run? Finished tasks are kept.</span>
              <Button variant="danger" onClick={() => void cancel()}>Yes, cancel</Button>
              <Button variant="ghost" onClick={() => setConfirming(false)}>Keep running</Button>
            </>
          ) : (
            <Button variant="danger" onClick={() => setConfirming(true)}>Cancel run</Button>
          )}
        </div>
      </div>

      <Panel>
        <div className="flex items-center gap-4">
          <div className="min-w-0 flex-1"><Bar value={progress} height={8} /></div>
          <div className="num w-12 text-right text-[15px] font-semibold">{pct(progress)}</div>
        </div>

        {!finished && (
          <div className="mt-4 grid gap-4 md:grid-cols-[1.4fr_1fr]">
            <div>
              <div className="num text-[12px] text-muted">
                Task {live.taskTitle ? live.taskIndex + 1 : "–"} / {live.tasksPerModel}
                {live.attempt > 1 && <span className="ml-2 text-warn">repair attempt {live.attempt}</span>}
              </div>
              <div className="mt-0.5 truncate text-[15px] font-medium">{live.taskTitle ?? "Waiting for the first task…"}</div>
              <div className="mt-2 flex items-baseline gap-3">
                <span className="num text-[13px] font-semibold tracking-[0.14em] text-accent2"><span className="live-dot mr-2 inline-block h-1.5 w-1.5 rounded-full bg-accent2" />{live.phase ? phaseLabel[live.phase] : "STARTING"}</span>
                <span className="num text-[13px] text-muted">{fixed(elapsed, 1)} sec</span>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-x-4 gap-y-3">
              <Stat label="GPU" value={sample?.gpuPercent != null ? `${sample.gpuPercent.toFixed(0)}%` : "unavailable"} />
              <Stat label="VRAM" value={sample?.vramUsedMb != null ? `${(sample.vramUsedMb / 1024).toFixed(1)} / ${((sample.vramTotalMb ?? 0) / 1024).toFixed(0)} GB` : "unavailable"} />
              <Stat label="Tokens streamed" value={g ? thousands(g.chunks) : dashChar} sub={g ? `${thousands(g.chars)} chars` : undefined} />
              <Stat label="Speed" value={approxRate ? `≈ ${approxRate.toFixed(1)} tok/s` : dashChar} sub="final value is provider-reported" tone="accent" />
              <Stat label="CPU" value={sample?.cpuPercent != null ? `${sample.cpuPercent.toFixed(0)}%` : "unavailable"} />
              <Stat label="RAM" value={sample ? mb(sample.ramUsedMb) : "unavailable"} />
            </div>
          </div>
        )}
        {!finished && live.telemetry.length > 4 && (
          <div className="mt-4 flex gap-8 border-t border-line pt-3 text-[11px] text-dim">
            <div>GPU % <Sparkline values={live.telemetry.slice(-90).map((s) => s.gpuPercent)} max={100} width={200} height={28} /></div>
            <div>VRAM <Sparkline values={live.telemetry.slice(-90).map((s) => s.vramUsedMb)} width={200} height={28} tone="accent" /></div>
            <div>CPU % <Sparkline values={live.telemetry.slice(-90).map((s) => s.cpuPercent)} max={100} width={200} height={28} tone="ok" /></div>
          </div>
        )}
      </Panel>

      <div className="mt-4 grid gap-4 lg:grid-cols-[1fr_1.35fr]">
        <Panel title="Models">
          <ul className="flex flex-col gap-3">
            {live.models.length === 0 && <li className="text-muted">Waiting…</li>}
            {live.models.map((m) => (
              <li key={m.key}>
                <div className="mb-1 flex items-center justify-between">
                  <span className="truncate font-medium">{m.displayName}</span>
                  <span className="num text-[12px] text-accent2">{m.done ? (m.score !== null ? thousands(m.score) : "DNF") : m.key === live.currentModel && !finished ? "running" : ""}</span>
                </div>
                <div className="flex flex-wrap gap-[3px]">
                  {Array.from({ length: live.tasksPerModel }, (_, i) => {
                    const t = m.tasks[i];
                    return <span key={i} title={t ? `${t.id}: ${t.status}${t.score !== null ? ` (${t.score.toFixed(0)})` : ""}` : "pending"} className={cx("h-3 w-3 rounded-[1px]", t ? statusColor[t.status] : "bg-line")} />;
                  })}
                </div>
              </li>
            ))}
          </ul>
        </Panel>

        <Panel title="Activity" actions={<Button variant="ghost" onClick={() => setShowAll((v) => !v)}>{showAll ? "Recent only" : "Full log"}</Button>}>
          <ul className={cx("flex flex-col gap-0.5 overflow-y-auto", showAll ? "max-h-[420px]" : "max-h-[280px]")} aria-live="polite">
            {logs.length === 0 ? <li className="text-muted">No activity yet.</li> : logs.map((e, i) => <LogLine key={`${e.ts}-${i}`} e={e} />)}
          </ul>
        </Panel>
      </div>
    </div>
  );
}

const dashChar = "—";
