import { Button, Dot, Spinner } from "../components/ui";
import { allModels } from "../components/model";
import { mb } from "../lib/format";
import { api } from "../ipc/commands";
import { useApp } from "../store/app";
import { useNav } from "../store/nav";

export function Welcome({ onDone }: { onDone: () => void }) {
  const { system, providers } = useApp();
  const go = useNav((s) => s.go);
  const models = allModels(providers);
  const gpu = system?.gpus[0];

  async function finish(startQuick: boolean) {
    await api.markOnboardingDone();
    onDone();
    if (startQuick) {
      const first = models[0];
      go({ page: "new", suiteId: "quick", preselect: first ? [`${first.providerId}/${first.id}`] : [] });
    }
  }

  return (
    <div className="grid-bg flex h-full items-center justify-center bg-ink p-6">
      <div className="w-full max-w-[560px] rounded-md border border-line2 bg-panel p-8 shadow-[0_0_80px_-30px_var(--color-accent)]">
        <div className="label mb-2 !text-accent">Welcome to PulseBench</div>
        <h1 className="text-[22px] font-semibold tracking-tight">Benchmark coding AI on your hardware.</h1>
        <p className="mt-1 text-muted">Real tasks, real test execution, reproducible scores — locally.</p>

        <div className="mt-6 space-y-5 text-[12.5px]">
          <section>
            <div className="label mb-1.5">System detected</div>
            {!system ? <Spinner label="Detecting…" /> : (
              <ul className="num space-y-0.5 text-muted">
                <li><span className="text-ok">✓</span> {gpu ? `${gpu.name}${gpu.vramTotalMb ? ` · ${mb(gpu.vramTotalMb)} VRAM` : ""}` : "No dedicated GPU detected (CPU inference will be slow)"}</li>
                <li><span className="text-ok">✓</span> {mb(system.ramTotalMb)} RAM</li>
                <li><span className="text-ok">✓</span> {system.osName} {system.osVersion}</li>
              </ul>
            )}
          </section>
          <section>
            <div className="label mb-1.5">AI providers</div>
            {!providers ? <Spinner label="Looking…" /> : (
              <ul className="space-y-1">
                {providers.map((p) => (
                  <li key={p.providerId} className="flex items-center gap-2">
                    <Dot state={p.state === "connected" ? "ok" : "off"} />
                    <span>{p.name}</span>
                    <span className="text-muted">{p.state === "connected" ? `detected · ${p.models.length} models` : "not detected"}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>
          <section>
            <div className="label mb-1.5">Installed models</div>
            {!providers ? null : models.length === 0 ? (
              <p className="text-muted">None yet. Install one (e.g. <span className="num text-text">ollama pull qwen2.5-coder:7b</span>) and re-detect.</p>
            ) : (
              <ul className="num space-y-0.5 text-muted">
                {models.slice(0, 6).map((m) => <li key={`${m.providerId}/${m.id}`}><span className="text-ok">✓</span> {m.id}</li>)}
                {models.length > 6 && <li className="text-dim">…and {models.length - 6} more</li>}
              </ul>
            )}
          </section>
        </div>

        <div className="mt-8 flex items-center gap-3">
          <Button variant="primary" className="px-5 py-2 tracking-wider" disabled={models.length === 0} onClick={() => void finish(true)}>RUN QUICK BENCHMARK</Button>
          <Button variant="ghost" onClick={() => void finish(false)}>Skip to dashboard</Button>
        </div>
      </div>
    </div>
  );
}
