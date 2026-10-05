import { useState } from "react";
import type { Attempt, CommandOutcome, TaskResult } from "@pulsebench/types";
import { Badge, Button, Pre, Tabs } from "./ui";
import { cx, fixed, ms, mb } from "../lib/format";

type Tab = "overview" | "prompt" | "output" | "diff" | "tests" | "metrics";

const failureText: Record<string, string> = {
  "provider-error": "Provider error", "generation-timeout": "Generation timed out", truncated: "Output truncated at the token limit",
  "protocol-failure": "Output could not be parsed", "no-valid-changes": "No valid file changes", "compile-failed": "Compile / type-check failed",
  "tests-failed": "Tests failed", "test-timeout": "Test run timed out", "no-tests-found": "No tests were found", "mutants-survived": "Tests did not detect broken implementations",
  "too-few-tests": "Too few tests written", cancelled: "Cancelled",
};

function DiffView({ diff }: { diff: string }) {
  if (!diff) return <p className="text-muted">The model changed no files.</p>;
  return (
    <pre className="num max-h-[480px] overflow-auto rounded-sm border border-line bg-ink p-0 text-[11.5px] leading-relaxed">
      {diff.split("\n").map((l, i) => (
        <div key={i} className={cx("whitespace-pre px-3", l.startsWith("+") && !l.startsWith("+++") && "bg-ok/10 text-ok", l.startsWith("-") && !l.startsWith("---") && "bg-bad/10 text-bad", l.startsWith("@@") && "text-accent2", (l.startsWith("---") || l.startsWith("+++")) && "text-muted")}>
          {l || " "}
        </div>
      ))}
    </pre>
  );
}

function Command({ title, c }: { title: string; c: CommandOutcome }) {
  return (
    <div className="mb-4">
      <div className="mb-1 flex items-center gap-2">
        <span className="label">{title}</span>
        <code className="num text-[11.5px] text-muted">{c.command}</code>
        {c.spawnError ? <Badge kind="bad">could not run</Badge> : c.timedOut ? <Badge kind="bad">timed out</Badge> : c.exitCode === 0 ? <Badge kind="ok">exit 0</Badge> : <Badge kind="bad">exit {c.exitCode}</Badge>}
        <span className="num text-[11px] text-dim">{ms(c.durationMs)}</span>
      </div>
      {c.spawnError && <p className="mb-1 text-bad">{c.spawnError}</p>}
      {c.stdout && <><div className="mb-0.5 text-[10.5px] uppercase tracking-wider text-dim">stdout</div><Pre>{c.stdout}</Pre></>}
      {c.stderr && <><div className="mb-0.5 mt-2 text-[10.5px] uppercase tracking-wider text-dim">stderr</div><Pre>{c.stderr}</Pre></>}
      {!c.stdout && !c.stderr && !c.spawnError && <p className="text-[11.5px] text-dim">(no output)</p>}
      {c.truncated && <p className="mt-1 text-[11px] text-warn">Output was truncated.</p>}
    </div>
  );
}

function AttemptView({ a, tab, task }: { a: Attempt; tab: Tab; task: TaskResult }) {
  const g = a.generation;
  if (tab === "overview") {
    return (
      <div className="space-y-3 text-[12.5px]">
        <div className="flex flex-wrap items-center gap-2">
          {a.solved ? <Badge kind="ok">solved</Badge> : <Badge kind="bad">not solved</Badge>}
          {a.failure && <Badge kind="bad">{failureText[a.failure] ?? a.failure}</Badge>}
          {a.parse && <Badge kind={a.parse.mode === "json" ? "ok" : a.parse.mode === "recovered" ? "warn" : "bad"}>output: {a.parse.mode === "json" ? "clean JSON" : a.parse.mode}</Badge>}
        </div>
        {a.error && <p className="text-bad">{a.error}</p>}
        {a.parse?.explanation && <div><div className="label mb-1">Model's explanation</div><p className="text-muted">{a.parse.explanation}</p></div>}
        {a.parse && a.parse.notes.length > 0 && <div><div className="label mb-1">Output recovery notes</div><ul className="list-disc pl-4 text-warn">{a.parse.notes.map((n, i) => <li key={i}>{n}</li>)}</ul></div>}
        <div>
          <div className="label mb-1">Files</div>
          {a.changes.length === 0 ? <p className="text-muted">No changes proposed.</p> : (
            <ul className="space-y-0.5">{a.changes.map((c, i) => <li key={i} className="num text-[12px]"><span className={c.status === "applied" ? "text-ok" : "text-bad"}>{c.status}</span> {c.path} <span className="text-dim">({c.bytes} B)</span>{c.reason && <span className="text-muted"> — {c.reason}</span>}</li>)}</ul>
          )}
        </div>
        {task.baseline && (
          <p className="text-[11.5px] text-dim">Baseline (untouched fixture): {task.baseline.tests?.passed || task.baseline.compile?.exitCode === 0 && !task.baseline.tests ? "passes" : "fails"} as expected{task.baseline.alreadyPassing ? " — warning: the task is suspect" : ""}.</p>
        )}
      </div>
    );
  }
  if (tab === "prompt") {
    return (
      <div className="space-y-3">
        <div><div className="label mb-1">System</div><Pre>{a.prompt.system}</Pre></div>
        <div><div className="label mb-1">Prompt · files supplied: <span className="normal-case tracking-normal text-muted">{a.prompt.files.join(", ")}</span></div><Pre>{a.prompt.user}</Pre></div>
      </div>
    );
  }
  if (tab === "output") {
    return (
      <div className="space-y-3">
        {g?.reasoning && <div><div className="label mb-1">Reasoning (not scored)</div><Pre className="max-h-48 text-muted">{g.reasoning}</Pre></div>}
        <div><div className="label mb-1">Raw model output</div>{g ? <Pre>{g.text || "(empty)"}</Pre> : <p className="text-muted">No output was produced.</p>}</div>
      </div>
    );
  }
  if (tab === "diff") return <DiffView diff={a.diff} />;
  if (tab === "tests") {
    return (
      <div>
        {a.compile && <Command title="Compile" c={a.compile} />}
        {a.tests && (
          <>
            <Command title="Tests" c={a.tests.command} />
            <p className="num mb-3 text-[12px] text-muted">{a.tests.summary.passed ?? "?"} passed · {a.tests.summary.failed ?? "?"} failed · {a.tests.summary.total ?? "?"} total{a.tests.summary.skipped ? ` · ${a.tests.summary.skipped} skipped` : ""}</p>
          </>
        )}
        {a.mutants.length > 0 && (
          <div>
            <div className="label mb-1">Defect detection ({a.mutants.filter((m) => m.killed).length}/{a.mutants.length})</div>
            <ul className="space-y-0.5">{a.mutants.map((m) => <li key={m.id} className="text-[12px]"><span className={m.killed ? "text-ok" : "text-bad"}>{m.killed ? "detected" : "MISSED"}</span> <span className="text-muted">{m.description}</span></li>)}</ul>
          </div>
        )}
        {!a.compile && !a.tests && <p className="text-muted">Nothing was executed for this attempt{a.failure ? ` (${failureText[a.failure] ?? a.failure})` : ""}.</p>}
      </div>
    );
  }
  const r = a.resources;
  const rows: Array<[string, string]> = [
    ["Prompt tokens", g?.promptTokens != null ? String(g.promptTokens) : "n/a"],
    ["Completion tokens", g?.completionTokens != null ? String(g.completionTokens) : "n/a"],
    ["Context tokens (total)", g?.totalTokens != null ? String(g.totalTokens) : "n/a"],
    ["Generation time", g ? ms(g.durationMs) : "n/a"],
    ["Time to first token", ms(g?.timeToFirstTokenMs)],
    ["Tokens / second", g?.tokensPerSecond != null ? `${g.tokensPerSecond.toFixed(1)} (${g.metricSource === "provider-reported" ? "provider-reported" : "client-measured"})` : "n/a"],
    ["Model load time", ms(g?.loadDurationMs)],
    ["Finish reason", g?.finishReason ?? "n/a"],
    ["Attempt duration", ms(a.durationMs)],
    ["Peak GPU utilization", r.peakGpuPercent != null ? `${r.peakGpuPercent.toFixed(0)}%` : "unavailable"],
    ["Peak VRAM", mb(r.peakVramMb)],
    ["Peak CPU", r.peakCpuPercent != null ? `${r.peakCpuPercent.toFixed(0)}%` : "unavailable"],
    ["Peak system RAM", mb(r.peakRamMb)],
    ["GPU temperature", r.peakGpuTempC != null ? `${fixed(r.peakGpuTempC, 0)} °C` : "unavailable"],
    ["Telemetry samples", String(r.samples)],
  ];
  return (
    <dl className="grid grid-cols-[1fr_auto] gap-x-6 gap-y-1.5 text-[12.5px]">
      {rows.map(([k, v]) => (<div key={k} className="contents"><dt className="text-muted">{k}</dt><dd className={cx("num text-right", v === "n/a" || v === "unavailable" ? "text-dim" : "")}>{v}</dd></div>))}
    </dl>
  );
}

export function TaskDrawer({ task, onClose }: { task: TaskResult; onClose: () => void }) {
  const [tab, setTab] = useState<Tab>("overview");
  const [attemptIdx, setAttemptIdx] = useState(task.attempts.length - 1);
  const attempt = task.attempts[Math.min(attemptIdx, task.attempts.length - 1)];
  return (
    <aside className="fixed inset-y-0 right-0 z-40 flex w-[min(760px,92vw)] flex-col border-l border-line2 bg-panel shadow-[-20px_0_60px_-20px_#000]" aria-label={`Task ${task.id}`}>
      <header className="flex items-start justify-between gap-3 border-b border-line px-4 py-3">
        <div className="min-w-0">
          <div className="num text-[11px] text-dim">{task.id} · {task.category} · {task.language} · {task.difficulty}</div>
          <h2 className="truncate text-[15px] font-semibold">{task.title}</h2>
          <div className="mt-1 flex items-center gap-2">
            <Badge kind={task.status === "passed" ? "ok" : task.status === "failed" ? "bad" : "warn"}>{task.status}</Badge>
            {task.score && <span className="num text-[12px] text-accent2">task score {task.score.value.toFixed(1)}</span>}
            {task.attempts.length > 1 && (
              <span className="flex items-center gap-1 text-[11.5px] text-muted">attempt
                {task.attempts.map((a, i) => <button key={a.number} onClick={() => setAttemptIdx(i)} className={cx("num h-5 w-5 rounded-sm border text-[11px]", i === attemptIdx ? "border-accent text-accent" : "border-line2 text-muted")}>{a.number}</button>)}
              </span>
            )}
          </div>
        </div>
        <Button variant="ghost" onClick={onClose} aria-label="Close task details">Close</Button>
      </header>
      <div className="px-4">
        <Tabs<Tab> tabs={[{ id: "overview", label: "Overview" }, { id: "prompt", label: "Prompt" }, { id: "output", label: "Raw output" }, { id: "diff", label: "Diff" }, { id: "tests", label: "Tests" }, { id: "metrics", label: "Metrics" }]} value={tab} onChange={setTab} />
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        {task.message && <p className="mb-3 rounded-sm border border-warn/40 bg-warn/10 p-2 text-[12px] text-warn">{task.message}</p>}
        {attempt ? <AttemptView a={attempt} tab={tab} task={task} /> : <p className="text-muted">This task did not run{task.message ? ` (${task.message})` : ""}.</p>}
      </div>
    </aside>
  );
}
