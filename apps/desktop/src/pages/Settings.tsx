import { useEffect, useState } from "react";
import type { AppSettings, ProviderConfig, RunSettings } from "@pulsebench/types";
import { Badge, Button, PageHeader, Panel } from "../components/ui";
import { api } from "../ipc/commands";
import { pickSavePath } from "../lib/dialogs";
import { cx } from "../lib/format";
import { useApp } from "../store/app";

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="grid grid-cols-[170px_1fr] items-center gap-3 py-1.5 text-[12.5px]">
      <span className="text-muted">{label}</span>
      <span className="flex flex-col gap-0.5">{children}{hint && <span className="text-[11px] text-dim">{hint}</span>}</span>
    </label>
  );
}

const input = "num w-full max-w-[360px] rounded-sm border border-line2 bg-ink px-2 py-1.5 text-[12px] focus:border-accent";

function NumberField({ value, onChange, min, max, step }: { value: number; onChange: (n: number) => void; min?: number; max?: number; step?: number }) {
  return <input className={cx(input, "max-w-[160px]")} type="number" value={value} min={min} max={max} step={step} onChange={(e) => onChange(Number(e.target.value))} />;
}

export function Settings() {
  const { settings, system, data, providers, setSettings, refreshProviders, notify } = useApp();
  const [draft, setDraft] = useState<AppSettings | null>(settings);
  const [defaults, setDefaults] = useState<RunSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);

  useEffect(() => { void api.defaultRunSettings().then(setDefaults); }, []);
  useEffect(() => { if (settings && !draft) setDraft(settings); }, [settings, draft]);
  if (!draft || !settings) return null;

  const dirty = JSON.stringify(draft) !== JSON.stringify(settings);
  const run = draft.run;
  const nonStandard = defaults && JSON.stringify({ g: run.generation, s: run.scoring, d: run.dockerImages }) !== JSON.stringify({ g: defaults.generation, s: defaults.scoring, d: defaults.dockerImages });
  const patchRun = (p: Partial<RunSettings>) => setDraft({ ...draft, run: { ...run, ...p } });
  const patchGen = (p: Partial<RunSettings["generation"]>) => patchRun({ generation: { ...run.generation, ...p } });
  const patchProvider = (i: number, p: Partial<ProviderConfig>) => setDraft({ ...draft, providers: draft.providers.map((x, j) => (j === i ? { ...x, ...p } : x)) });

  async function save() {
    setSaving(true);
    try {
      const saved = await api.saveSettings(draft!);
      setSettings(saved);
      setDraft(saved);
      notify("ok", "Settings saved");
      void refreshProviders();
    } catch (e) {
      notify("error", e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  async function exportDb() {
    try {
      const path = await pickSavePath("pulsebench-backup.db");
      if (!path) return;
      await api.exportDatabase(path);
      notify("ok", `Database exported to ${path}`);
    } catch (e) {
      notify("error", String(e));
    }
  }

  async function clearHistory() {
    try {
      const n = await api.clearHistory();
      setConfirmClear(false);
      await useApp.getState().refreshRuns();
      notify("ok", `Deleted ${n} runs`);
    } catch (e) {
      notify("error", String(e));
    }
  }

  const w = run.scoring.weights;
  const weightKeys = ["correctness", "tests", "compile", "efficiency", "speed", "reliability"] as const;
  const wsum = weightKeys.reduce((n, k) => n + w[k], 0);

  return (
    <div className="mx-auto max-w-[900px] pb-20">
      <PageHeader title="Settings" actions={<><Button disabled={!dirty} onClick={() => setDraft(settings)}>Revert</Button><Button variant="primary" disabled={!dirty || saving} onClick={() => void save()}>{saving ? "Saving…" : "Save changes"}</Button></>} />

      <Panel title="Providers" actions={<Button variant="ghost" onClick={() => setDraft({ ...draft, providers: [...draft.providers, { id: `custom-${draft.providers.length + 1}`, kind: "openai-compatible", name: "Custom endpoint", baseUrl: "http://localhost:8000/v1", enabled: true }] })}>+ Add OpenAI-compatible endpoint</Button>}>
        <ul className="divide-y divide-line">
          {draft.providers.map((p, i) => {
            const st = providers?.find((x) => x.providerId === p.id);
            const builtin = p.id === "ollama" || p.id === "lmstudio";
            return (
              <li key={p.id} className="py-3">
                <div className="mb-1 flex items-center gap-2">
                  <input type="checkbox" aria-label={`Enable ${p.name}`} checked={p.enabled} onChange={(e) => patchProvider(i, { enabled: e.target.checked })} className="accent-[var(--color-accent)]" />
                  {builtin ? <span className="font-semibold">{p.name}</span> : <input className={cx(input, "max-w-[220px] font-sans")} value={p.name} onChange={(e) => patchProvider(i, { name: e.target.value })} aria-label="Provider name" />}
                  <Badge kind={st?.state === "connected" ? "ok" : st?.state === "disabled" || !st ? "muted" : "bad"}>{st?.state ?? "unknown"}</Badge>
                  {!builtin && <Button variant="ghost" onClick={() => setDraft({ ...draft, providers: draft.providers.filter((_, j) => j !== i) })}>Remove</Button>}
                </div>
                <Field label="Base URL"><input className={input} value={p.baseUrl} onChange={(e) => patchProvider(i, { baseUrl: e.target.value })} aria-label={`${p.name} URL`} /></Field>
                {p.kind === "openai-compatible" && <Field label="API key (optional)" hint="Stored locally in the PulseBench database; never included in exports."><input className={input} type="password" value={p.apiKey ?? ""} onChange={(e) => patchProvider(i, { apiKey: e.target.value })} autoComplete="off" /></Field>}
                {st?.error && <p className="text-[11.5px] text-bad">{st.error}</p>}
              </li>
            );
          })}
        </ul>
      </Panel>

      <Panel className="mt-4" title="Benchmark" actions={nonStandard ? <Badge kind="warn">non-standard</Badge> : <Badge kind="ok">standard</Badge>}>
        <p className="mb-2 rounded-sm border border-warn/30 bg-warn/5 p-2 text-[11.5px] text-warn/90">The defaults are standardized so results are comparable. Changing any value below makes results less directly comparable with other runs; PulseBench records the exact configuration with every result.</p>
        <Field label="Temperature" hint="0 = greedy decoding (recommended)"><NumberField value={run.generation.temperature} min={0} max={2} step={0.1} onChange={(n) => patchGen({ temperature: n })} /></Field>
        <Field label="Seed" hint="Empty = no seed"><input className={cx(input, "max-w-[160px]")} type="number" value={run.generation.seed ?? ""} onChange={(e) => patchGen({ seed: e.target.value === "" ? null : Number(e.target.value) })} /></Field>
        <Field label="Context limit (tokens)" hint="Applied by Ollama. LM Studio and other OpenAI-compatible servers use the context the model was loaded with."><NumberField value={run.generation.contextTokens} min={512} step={1024} onChange={(n) => patchGen({ contextTokens: n })} /></Field>
        <Field label="Generation limit (tokens)" hint="Thinking models may need a higher limit"><NumberField value={run.generation.maxOutputTokens} min={64} step={512} onChange={(n) => patchGen({ maxOutputTokens: n })} /></Field>
        <Field label="Generation timeout (s)"><NumberField value={run.generation.timeoutSeconds} min={5} step={30} onChange={(n) => patchGen({ timeoutSeconds: n })} /></Field>
        <Field label="Parallelism"><span className="text-muted">1 (serial) — fixed. Models and tasks run one at a time so timing and VRAM measurements are fair.</span></Field>
        <details className="mt-2">
          <summary className="cursor-pointer text-[12px] text-muted hover:text-text">Scoring weights (advanced) — {run.scoring.formula}</summary>
          <div className="mt-2 grid grid-cols-2 gap-x-6">
            {weightKeys.map((k) => (
              <Field key={k} label={k[0]!.toUpperCase() + k.slice(1)} hint={`${wsum > 0 ? ((w[k] / wsum) * 100).toFixed(0) : 0}% of the score`}>
                <NumberField value={w[k]} min={0} step={1} onChange={(n) => patchRun({ scoring: { ...run.scoring, weights: { ...w, [k]: n } } })} />
              </Field>
            ))}
            <Field label="Speed floor (tok/s)" hint="maps to a speed component of 0"><NumberField value={run.scoring.speedFloorTps} min={0.1} onChange={(n) => patchRun({ scoring: { ...run.scoring, speedFloorTps: n } })} /></Field>
            <Field label="Speed ceiling (tok/s)" hint="maps to a speed component of 1"><NumberField value={run.scoring.speedCeilingTps} min={1} onChange={(n) => patchRun({ scoring: { ...run.scoring, speedCeilingTps: n } })} /></Field>
          </div>
        </details>
        <div className="mt-3"><Button variant="ghost" disabled={!defaults} onClick={() => defaults && patchRun({ generation: defaults.generation, scoring: defaults.scoring, dockerImages: defaults.dockerImages })}>Reset benchmark settings to defaults</Button></div>
      </Panel>

      <Panel className="mt-4" title="Runtime">
        <Field label="Execution mode" hint={run.execution === "docker" ? "Generated code runs in `docker run --network none` with only the workspace mounted." : "A disposable directory, sanitized environment, command allowlist and timeouts. This is not an OS-level security boundary; use Docker for real isolation."}>
          <span className="flex gap-4">
            <label className="flex items-center gap-1.5"><input type="radio" name="exec" checked={run.execution === "local"} onChange={() => patchRun({ execution: "local" })} className="accent-[var(--color-accent)]" /> Local isolated workspace</label>
            <label className={cx("flex items-center gap-1.5", !system?.docker.running && "opacity-60")}><input type="radio" name="exec" disabled={!system?.docker.running} checked={run.execution === "docker"} onChange={() => patchRun({ execution: "docker" })} className="accent-[var(--color-accent)]" /> Docker {!system?.docker.installed ? "(not installed)" : !system.docker.running ? "(daemon not running)" : ""}</label>
          </span>
        </Field>
        {run.execution === "docker" && (<>
          <Field label="Python image"><input className={input} value={run.dockerImages.python} onChange={(e) => patchRun({ dockerImages: { ...run.dockerImages, python: e.target.value } })} /></Field>
          <Field label="Node image"><input className={input} value={run.dockerImages.node} onChange={(e) => patchRun({ dockerImages: { ...run.dockerImages, node: e.target.value } })} /></Field>
        </>)}
        <Field label="Keep workspaces" hint="Leave the temporary workspaces on disk after a run (for debugging)."><input type="checkbox" checked={draft.keepWorkspaces} onChange={(e) => setDraft({ ...draft, keepWorkspaces: e.target.checked })} className="accent-[var(--color-accent)]" /></Field>
      </Panel>

      <Panel className="mt-4" title="Data">
        <dl className="grid grid-cols-[170px_1fr] gap-y-1.5 text-[12px]">
          <dt className="text-dim">Data directory</dt><dd className="num break-all">{data?.dataDir}</dd>
          <dt className="text-dim">Database</dt><dd className="num break-all">{data?.databasePath} <span className="text-dim">(schema v{data?.schemaVersion})</span></dd>
          <dt className="text-dim">Custom suites</dt><dd className="num break-all">{data?.userSuitesDir}</dd>
          <dt className="text-dim">Version</dt><dd className="num">PulseBench {data?.appVersion}</dd>
        </dl>
        <div className="mt-3 flex gap-2">
          <Button onClick={() => void exportDb()}>Export database…</Button>
          {confirmClear ? (<><span className="self-center text-bad">Delete all benchmark history?</span><Button variant="danger" onClick={() => void clearHistory()}>Yes, delete everything</Button><Button variant="ghost" onClick={() => setConfirmClear(false)}>Cancel</Button></>) : <Button variant="danger" onClick={() => setConfirmClear(true)}>Clear history…</Button>}
        </div>
        <p className="mt-3 text-[11.5px] text-dim">PulseBench sends nothing anywhere: no telemetry, no uploads. Your source code and results stay on this machine.</p>
      </Panel>
    </div>
  );
}
