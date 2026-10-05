import { useEffect, useMemo, useState } from "react";
import type { Runtime, SuiteSummary } from "@pulsebench/types";
import { Badge, Button, Empty, PageHeader, Panel } from "../components/ui";
import { ModelMeta, allModels, modelKeyOf } from "../components/model";
import { cx } from "../lib/format";
import { api } from "../ipc/commands";
import { useApp } from "../store/app";
import { useNav } from "../store/nav";

const runtimeName: Record<Runtime, string> = { python: "Python 3", node: "Node.js" };

function suiteWarnings(suite: SuiteSummary, system: ReturnType<typeof useApp.getState>["system"], docker: boolean): string[] {
  if (!system) return [];
  const out: string[] = [];
  if (docker) {
    if (!system.docker.running) out.push("Docker execution is selected but the Docker daemon is not reachable; tasks would be skipped.");
    return out;
  }
  for (const rt of suite.requires) {
    const found = system.runtimes.find((r) => r.name === runtimeName[rt]);
    if (!found?.available) out.push(`${runtimeName[rt]} was not found on this machine; ${rt === "python" ? "Python" : "JavaScript/TypeScript/React"} tasks will be skipped.`);
  }
  if (suite.requires.includes("node") && !system.runtimes.find((r) => r.name === "npm")?.available) out.push("npm was not found; the one-time TypeScript/React toolchain install will fail.");
  return out;
}

export function NewBenchmark() {
  const { providers, suites, settings, system, refreshProviders, notify, startLive } = useApp();
  const route = useNav((s) => s.route);
  const go = useNav((s) => s.go);
  const models = allModels(providers);
  const [selected, setSelected] = useState<string[]>(route.page === "new" ? route.preselect ?? [] : []);
  const [suiteId, setSuiteId] = useState<string>(route.page === "new" && route.suiteId ? route.suiteId : "quick");
  const [subset, setSubset] = useState<string[] | null>(null);
  const [starting, setStarting] = useState(false);
  const [standard, setStandard] = useState<boolean | null>(null);

  const suite = suites?.suites.find((s) => s.id === suiteId) ?? suites?.suites[0];
  useEffect(() => {
    if (suites && !suites.suites.some((s) => s.id === suiteId) && suites.suites[0]) setSuiteId(suites.suites[0].id);
  }, [suites, suiteId]);
  useEffect(() => {
    if (!settings) return;
    void api.defaultRunSettings().then((d) => setStandard(JSON.stringify({ g: settings.run.generation, s: settings.run.scoring, d: settings.run.dockerImages }) === JSON.stringify({ g: d.generation, s: d.scoring, d: d.dockerImages })));
  }, [settings]);

  const docker = settings?.run.execution === "docker";
  const warnings = useMemo(() => (suite ? suiteWarnings(suite, system, docker) : []), [suite, system, docker]);
  const canStart = selected.length > 0 && !!suite && !starting;

  function toggle(key: string) {
    setSelected((s) => (s.includes(key) ? s.filter((k) => k !== key) : [...s, key]));
  }

  async function start() {
    if (!suite) return;
    setStarting(true);
    try {
      const picks = selected.map((k) => {
        const m = models.find((x) => modelKeyOf(x) === k)!;
        return { providerId: m.providerId, modelId: m.id };
      });
      const runId = await api.startRun({ suiteId: suite.id, models: picks, taskIds: subset && subset.length < suite.taskCount ? subset : null, settings: null });
      startLive(runId);
      go({ page: "live" });
    } catch (e) {
      notify("error", e instanceof Error ? e.message : String(e));
    } finally {
      setStarting(false);
    }
  }

  return (
    <div className="mx-auto max-w-[1100px]">
      <PageHeader title="New benchmark" sub="Every selected model receives byte-identical prompts, files, limits and tests." />

      <div className="grid gap-4 lg:grid-cols-[1.1fr_1fr]">
        <Panel title={`1 · Models (${selected.length} selected)`} actions={<Button variant="ghost" onClick={() => void refreshProviders()}>Re-detect</Button>}>
          {models.length === 0 ? (
            <Empty title="No models available">Start Ollama or LM Studio and install a model, then re-detect.</Empty>
          ) : (
            <ul className="flex max-h-[420px] flex-col gap-1 overflow-y-auto">
              {models.map((m) => {
                const k = modelKeyOf(m);
                const on = selected.includes(k);
                return (
                  <li key={k}>
                    <label className={cx("flex cursor-pointer items-center gap-3 rounded-sm border px-3 py-2", on ? "border-accent/60 bg-accent/10" : "border-line hover:border-line2")}>
                      <input type="checkbox" checked={on} onChange={() => toggle(k)} className="accent-[var(--color-accent)]" aria-label={`Select ${m.displayName}`} />
                      <div className="min-w-0 flex-1">
                        <div className="truncate font-medium">{m.displayName}</div>
                        <ModelMeta m={m} />
                      </div>
                      <Badge>{m.providerId}</Badge>
                    </label>
                  </li>
                );
              })}
            </ul>
          )}
        </Panel>

        <div className="flex flex-col gap-4">
          <Panel title="2 · Suite">
            <div className="flex flex-col gap-2">
              {suites?.suites.map((s) => (
                <button key={s.id} onClick={() => { setSuiteId(s.id); setSubset(null); }} className={cx("rounded-sm border p-3 text-left", suite?.id === s.id ? "border-accent/60 bg-accent/10" : "border-line hover:border-line2")}>
                  <div className="flex items-center justify-between">
                    <span className="font-semibold">{s.name} <span className="num font-normal text-dim">v{s.version}</span></span>
                    <span className="flex gap-1.5">{s.official ? <Badge kind="accent">official</Badge> : <Badge kind="warn">custom</Badge>}<Badge>{s.taskCount} tasks</Badge></span>
                  </div>
                  <p className="mt-1 text-[11.5px] text-muted">{s.description}</p>
                  {s.estimatedMinutes && <p className="num mt-1 text-[11px] text-dim">≈ {s.estimatedMinutes} min per model on typical hardware</p>}
                </button>
              ))}
              {suites?.problems.map((p) => <p key={p.path} className="text-[11.5px] text-bad">Invalid suite {p.path}: {p.error}</p>)}
            </div>
          </Panel>

          <Panel title="3 · Configure">
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-[12px]">
              <dt className="text-dim">Execution</dt>
              <dd>{docker ? "Docker (network disabled)" : "Local isolated workspace"} <button className="ml-1 text-accent hover:underline" onClick={() => go({ page: "settings" })}>change</button></dd>
              <dt className="text-dim">Temperature</dt><dd className="num">{settings?.run.generation.temperature}</dd>
              <dt className="text-dim">Seed</dt><dd className="num">{settings?.run.generation.seed ?? "none"}</dd>
              <dt className="text-dim">Context / output</dt><dd className="num">{settings?.run.generation.contextTokens.toLocaleString()} / {settings?.run.generation.maxOutputTokens.toLocaleString()} tokens</dd>
              <dt className="text-dim">Timeout</dt><dd className="num">{settings?.run.generation.timeoutSeconds}s per generation</dd>
              <dt className="text-dim">Order</dt><dd>Models run one after another (serial) for fair timing</dd>
            </dl>
            {standard === false && <p className="mt-3 rounded-sm border border-warn/40 bg-warn/10 p-2 text-[11.5px] text-warn">Non-standard settings: results will not be directly comparable with default-configuration runs.</p>}
            {suite && (
              <details className="mt-3 text-[12px]">
                <summary className="cursor-pointer text-muted hover:text-text">Advanced: choose tasks ({(subset ?? suite.tasks.map((t) => t.id)).length}/{suite.taskCount})</summary>
                <ul className="mt-2 max-h-40 space-y-0.5 overflow-y-auto">
                  {suite.tasks.map((t) => {
                    const cur = subset ?? suite.tasks.map((x) => x.id);
                    return (
                      <li key={t.id}>
                        <label className="flex items-center gap-2">
                          <input type="checkbox" checked={cur.includes(t.id)} onChange={() => setSubset(cur.includes(t.id) ? cur.filter((x) => x !== t.id) : [...cur, t.id])} className="accent-[var(--color-accent)]" />
                          <span className="num text-dim">{t.id}</span> <span className="truncate">{t.title}</span>
                        </label>
                      </li>
                    );
                  })}
                </ul>
                <p className="mt-1 text-[11px] text-dim">Running a subset marks the result as non-standard.</p>
              </details>
            )}
            {suite?.requires.includes("node") && <p className="mt-3 text-[11.5px] text-muted">The first TypeScript/React run downloads a pinned toolchain (~50 MB, needs network once).</p>}
            {warnings.map((w) => <p key={w} className="mt-2 rounded-sm border border-warn/40 bg-warn/10 p-2 text-[11.5px] text-warn">{w}</p>)}
          </Panel>

          <Button variant="primary" className="py-2.5 text-[13px] tracking-wider" disabled={!canStart} onClick={() => void start()}>
            {starting ? "STARTING…" : `START BENCHMARK${selected.length ? ` · ${selected.length} MODEL${selected.length > 1 ? "S" : ""}` : ""}`}
          </Button>
          {selected.length === 0 && <p className="-mt-2 text-[11.5px] text-dim">Select at least one model.</p>}
        </div>
      </div>
    </div>
  );
}
