import type { ModelInfo, ProviderStatus } from "@pulsebench/types";
import { bytes, thousands, dash } from "../lib/format";
import { Badge } from "./ui";

export function modelKeyOf(m: ModelInfo): string {
  return `${m.providerId}/${m.id}`;
}

export function allModels(providers: ProviderStatus[] | null): ModelInfo[] {
  return (providers ?? []).flatMap((p) => p.models);
}

export function ModelMeta({ m }: { m: ModelInfo }) {
  const bits = [m.parameterSize, m.quantization, m.sizeBytes !== null ? bytes(m.sizeBytes) : null, m.contextLength !== null ? `${m.contextLength.toLocaleString("en-US")} ctx` : null].filter(Boolean);
  return <span className="num text-[11.5px] text-muted">{bits.length ? bits.join(" · ") : dash}</span>;
}

export function ModelCard({ model, score, suite, onClick, children }: { model: ModelInfo; score?: number; suite?: string; onClick?: () => void; children?: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-2 rounded-md border border-line bg-panel p-3.5 hover:border-line2" onClick={onClick}>
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="truncate text-[13.5px] font-semibold" title={model.id}>{model.displayName}</div>
          <ModelMeta m={model} />
        </div>
        <Badge kind={model.loaded ? "ok" : "muted"}>{model.providerId}</Badge>
      </div>
      <dl className="grid grid-cols-2 gap-x-3 gap-y-1 text-[11.5px]">
        <dt className="text-dim">Architecture</dt>
        <dd className="num truncate text-right text-muted">{model.architecture ?? dash}</dd>
        <dt className="text-dim">Latest score</dt>
        <dd className="num text-right font-semibold text-accent2" title={suite}>{score !== undefined ? thousands(score) : "not benchmarked"}</dd>
      </dl>
      {children}
    </div>
  );
}
