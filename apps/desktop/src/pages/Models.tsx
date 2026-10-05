import { Badge, Button, Empty, PageHeader, Panel, Spinner } from "../components/ui";
import { allModels, modelKeyOf } from "../components/model";
import { bytes, dash, thousands } from "../lib/format";
import { useApp } from "../store/app";
import { useNav } from "../store/nav";

export function Models() {
  const { providers, latest, refreshProviders, providersLoading } = useApp();
  const go = useNav((s) => s.go);
  const latestBy = new Map(latest.map((l) => [l.modelKey, l]));
  const models = allModels(providers);
  return (
    <div className="mx-auto max-w-[1180px]">
      <PageHeader title="Models" sub="Discovered live from your local providers. Nothing here is hard-coded." actions={<Button onClick={() => void refreshProviders()} disabled={providersLoading}>{providersLoading ? "Detecting…" : "Re-detect"}</Button>} />
      {!providers ? <Spinner label="Detecting providers…" /> : models.length === 0 ? (
        <Empty title="No models found">Start Ollama (<span className="num">http://localhost:11434</span>) or LM Studio's local server (<span className="num">http://localhost:1234</span>) and install a model.</Empty>
      ) : (
        <Panel pad={false}>
          <table className="w-full text-[12.5px]">
            <thead><tr className="border-b border-line text-left">
              {["Model", "Provider", "Params", "Quant", "Disk", "Context", "Arch", "Latest score", ""].map((h) => <th key={h} className="label px-3 py-2">{h}</th>)}
            </tr></thead>
            <tbody>
              {models.map((m) => {
                const l = latestBy.get(modelKeyOf(m));
                return (
                  <tr key={modelKeyOf(m)} className="border-b border-line/60 last:border-0 hover:bg-panel2">
                    <td className="px-3 py-2 font-medium">{m.displayName} {m.loaded && <Badge kind="ok">loaded</Badge>}</td>
                    <td className="px-3 py-2 text-muted">{m.providerId}</td>
                    <td className="num px-3 py-2">{m.parameterSize ?? dash}</td>
                    <td className="num px-3 py-2">{m.quantization ?? dash}</td>
                    <td className="num px-3 py-2">{m.sizeBytes !== null ? bytes(m.sizeBytes) : dash}</td>
                    <td className="num px-3 py-2">{m.contextLength !== null ? m.contextLength.toLocaleString("en-US") : dash}</td>
                    <td className="num px-3 py-2 text-muted">{m.architecture ?? dash}</td>
                    <td className="num px-3 py-2 text-accent2">{l ? thousands(l.score) : <span className="text-dim">—</span>}</td>
                    <td className="px-3 py-2 text-right"><Button variant="ghost" onClick={() => go({ page: "new", preselect: [modelKeyOf(m)] })}>Benchmark</Button></td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </Panel>
      )}
      {providers?.filter((p) => p.state !== "connected" && p.state !== "disabled").map((p) => (
        <p key={p.providerId} className="mt-3 text-[12px] text-muted"><span className="text-bad">{p.name}:</span> {p.error}</p>
      ))}
    </div>
  );
}
