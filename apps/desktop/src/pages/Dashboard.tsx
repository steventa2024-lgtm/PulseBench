import { Badge, Button, Dot, Empty, PageHeader, Panel, Spinner, Stat } from "../components/ui";
import { ModelCard, allModels, modelKeyOf } from "../components/model";
import { dateTime, mb, thousands } from "../lib/format";
import { useApp } from "../store/app";
import { useNav } from "../store/nav";

export function Dashboard() {
  const { system, providers, providersLoading, latest, runs, refreshProviders } = useApp();
  const go = useNav((s) => s.go);
  const models = allModels(providers);
  const gpu = system?.gpus[0];
  const latestBy = new Map(latest.map((l) => [l.modelKey, l]));

  return (
    <div className="mx-auto max-w-[1180px]">
      <PageHeader
        title="Dashboard"
        sub="Real coding benchmarks. On your hardware. With your models."
        actions={<Button variant="primary" onClick={() => go({ page: "new" })}>New benchmark</Button>}
      />

      <div className="grid gap-4 lg:grid-cols-[1.25fr_1fr]">
        <Panel title="System" actions={<Button variant="ghost" onClick={() => go({ page: "hardware" })}>Details</Button>}>
          {!system ? (
            <Spinner label="Detecting hardware…" />
          ) : (
            <div className="grid grid-cols-2 gap-x-6 gap-y-4 sm:grid-cols-3">
              <Stat label="GPU" value={gpu?.name ?? "None detected"} />
              <Stat label="VRAM" value={mb(gpu?.vramTotalMb)} sub={gpu?.vramUsedMb != null ? `${mb(gpu.vramTotalMb && gpu.vramUsedMb ? gpu.vramTotalMb - gpu.vramUsedMb : null)} available` : undefined} />
              <Stat label="CPU" value={system.cpuModel.replace(/\(R\)|\(TM\)/g, "").replace(/\s+/g, " ")} sub={`${system.cpuLogicalCores} threads`} />
              <Stat label="RAM" value={mb(system.ramTotalMb)} sub={`${mb(system.ramAvailableMb)} available`} />
              <Stat label="OS" value={`${system.osName} ${system.osVersion}`} />
              <Stat label="Architecture" value={system.arch} />
            </div>
          )}
        </Panel>

        <Panel title="Providers" actions={<Button variant="ghost" onClick={() => void refreshProviders()} disabled={providersLoading}>{providersLoading ? "Checking…" : "Re-detect"}</Button>}>
          {!providers ? (
            <Spinner label="Looking for local providers…" />
          ) : (
            <ul className="flex flex-col gap-3">
              {providers.map((p) => (
                <li key={p.providerId} className="flex items-start gap-3">
                  <span className="mt-1.5"><Dot state={p.state === "connected" ? "ok" : p.state === "disabled" ? "off" : "bad"} /></span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="label !text-text">{p.name}</span>
                      <span className="text-[11.5px] text-muted">
                        {p.state === "connected" ? `Connected${p.version ? ` · v${p.version}` : ""} · ${p.models.length} models` : p.state === "disabled" ? "Disabled" : p.state === "unreachable" ? "Not detected" : "Error"}
                      </span>
                    </div>
                    <div className="num truncate text-[11px] text-dim">{p.baseUrl}</div>
                    {p.error && <div className="mt-0.5 text-[11.5px] text-bad">{p.error}</div>}
                  </div>
                </li>
              ))}
            </ul>
          )}
        </Panel>
      </div>

      <h2 className="label mb-2 mt-6">Installed models</h2>
      {!providers ? (
        <Spinner label="Discovering models…" />
      ) : models.length === 0 ? (
        <Empty title="No models found" action={<Button onClick={() => go({ page: "settings" })}>Provider settings</Button>}>
          Install a coding model with Ollama (for example <span className="num text-text">ollama pull qwen2.5-coder:7b</span>) or load one in LM Studio and start its local server, then re-detect.
        </Empty>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {models.map((m) => {
            const l = latestBy.get(modelKeyOf(m));
            return (
              <ModelCard key={modelKeyOf(m)} model={m} score={l?.score} suite={l?.suiteName}>
                <Button className="mt-1" onClick={() => go({ page: "new", preselect: [modelKeyOf(m)] })}>Benchmark this model</Button>
              </ModelCard>
            );
          })}
        </div>
      )}

      <h2 className="label mb-2 mt-6">Recent runs</h2>
      {runs.length === 0 ? (
        <p className="text-muted">No runs yet. Your results are stored locally and stay available after you close PulseBench.</p>
      ) : (
        <Panel pad={false}>
          <ul className="divide-y divide-line">
            {runs.slice(0, 5).map((r) => {
              const best = [...r.models].filter((m) => m.score !== null).sort((a, b) => (b.score ?? 0) - (a.score ?? 0))[0];
              return (
                <li key={r.id}>
                  <button onClick={() => go({ page: "results", runId: r.id })} className="flex w-full items-center justify-between gap-4 px-3.5 py-2.5 text-left hover:bg-panel2">
                    <div>
                      <div className="font-medium">{r.suiteName} <span className="text-dim">v{r.suiteVersion}</span></div>
                      <div className="text-[11.5px] text-muted">{dateTime(r.createdAt)} · {r.models.length} models</div>
                    </div>
                    <div className="flex items-center gap-3">
                      {r.status !== "completed" && <Badge kind={r.status === "cancelled" || r.status === "interrupted" ? "warn" : "bad"}>{r.status}</Badge>}
                      {best && <span className="num text-[12px] text-muted">{best.displayName} <span className="font-semibold text-accent2">{thousands(best.score ?? 0)}</span></span>}
                    </div>
                  </button>
                </li>
              );
            })}
          </ul>
        </Panel>
      )}
    </div>
  );
}
