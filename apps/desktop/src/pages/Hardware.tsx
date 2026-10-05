import { useEffect, useState } from "react";
import type { TelemetrySample } from "@pulsebench/types";
import { Badge, Button, PageHeader, Panel, Sparkline, Spinner, Stat } from "../components/ui";
import { api } from "../ipc/commands";
import { dash, mb } from "../lib/format";
import { useApp } from "../store/app";

export function Hardware() {
  const { system, notify } = useApp();
  const [samples, setSamples] = useState<TelemetrySample[]>([]);
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    let alive = true;
    const tick = async () => {
      try {
        const s = await api.liveSample();
        if (alive) setSamples((a) => [...a.filter((x) => x.ts !== s.ts), s].slice(-120));
      } catch {
        /* backend busy */
      }
    };
    void tick();
    const t = setInterval(() => void tick(), 1500);
    return () => { alive = false; clearInterval(t); };
  }, []);

  async function refresh() {
    setRefreshing(true);
    try {
      useApp.setState({ system: await api.systemInfo(true) });
    } catch (e) {
      notify("error", String(e));
    } finally {
      setRefreshing(false);
    }
  }

  if (!system) return <Spinner label="Detecting hardware…" />;
  const last = samples.at(-1);
  return (
    <div className="mx-auto max-w-[1100px]">
      <PageHeader title="Hardware" sub="Detected on this machine. Metrics that cannot be measured are shown as unavailable, never estimated." actions={<Button onClick={() => void refresh()} disabled={refreshing}>{refreshing ? "Detecting…" : "Re-detect"}</Button>} />
      <div className="grid gap-4 lg:grid-cols-2">
        <Panel title="Live utilization">
          <div className="grid grid-cols-2 gap-4">
            <div><Stat label="CPU" value={last?.cpuPercent != null ? `${last.cpuPercent.toFixed(0)}%` : dash} /><Sparkline values={samples.map((s) => s.cpuPercent)} max={100} width={220} tone="ok" /></div>
            <div><Stat label="System RAM" value={last ? `${mb(last.ramUsedMb)} / ${mb(last.ramTotalMb)}` : dash} /><Sparkline values={samples.map((s) => s.ramUsedMb)} max={last?.ramTotalMb} width={220} tone="accent" /></div>
            <div><Stat label="GPU" value={last?.gpuPercent != null ? `${last.gpuPercent.toFixed(0)}%` : "unavailable"} /><Sparkline values={samples.map((s) => s.gpuPercent)} max={100} width={220} /></div>
            <div><Stat label="VRAM" value={last?.vramUsedMb != null ? `${mb(last.vramUsedMb)} / ${mb(last.vramTotalMb)}` : "unavailable"} sub={last?.gpuTempC != null ? `${last.gpuTempC.toFixed(0)} °C` : undefined} /><Sparkline values={samples.map((s) => s.vramUsedMb)} max={last?.vramTotalMb ?? undefined} width={220} tone="accent" /></div>
          </div>
        </Panel>
        <Panel title="Processor & memory">
          <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-[12.5px]">
            <dt className="text-dim">CPU</dt><dd>{system.cpuModel}</dd>
            <dt className="text-dim">Cores / threads</dt><dd className="num">{system.cpuPhysicalCores ?? dash} / {system.cpuLogicalCores}</dd>
            <dt className="text-dim">RAM</dt><dd className="num">{mb(system.ramTotalMb)} ({mb(system.ramAvailableMb)} free)</dd>
            <dt className="text-dim">OS</dt><dd>{system.osName} {system.osVersion} · {system.arch}</dd>
          </dl>
        </Panel>
        <Panel title="Graphics">
          {system.gpus.length === 0 ? <p className="text-muted">No GPU detected. Local models will run on the CPU.</p> : (
            <ul className="space-y-3">
              {system.gpus.map((g) => (
                <li key={g.name}>
                  <div className="flex items-center gap-2 font-medium">{g.name} <Badge kind={g.telemetryAvailable ? "ok" : "muted"}>{g.telemetryAvailable ? "live telemetry" : "no live telemetry"}</Badge></div>
                  <div className="num text-[12px] text-muted">VRAM {mb(g.vramTotalMb)} · driver {g.driverVersion ?? dash} · {g.vendor}</div>
                  {!g.telemetryAvailable && <div className="text-[11.5px] text-dim">Utilization and VRAM sampling needs nvidia-smi (NVIDIA GPUs). Other vendors are listed by name only.</div>}
                </li>
              ))}
            </ul>
          )}
        </Panel>
        <Panel title="Runtimes used to execute benchmark tasks">
          <ul className="space-y-1.5 text-[12.5px]">
            {system.runtimes.map((r) => (
              <li key={r.name} className="flex items-center justify-between"><span>{r.name}</span><span className="num text-muted">{r.available ? r.version : <span className="text-bad">not found</span>}</span></li>
            ))}
            <li className="flex items-center justify-between border-t border-line pt-1.5"><span>Docker</span><span className="num text-muted">{!system.docker.installed ? "not installed" : system.docker.running ? `running ${system.docker.version ?? ""}` : "installed, daemon not reachable"}</span></li>
          </ul>
        </Panel>
      </div>
    </div>
  );
}
