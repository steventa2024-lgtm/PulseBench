import { useEffect, useState } from "react";
import { api } from "../ipc/commands";
import { savePng, saveExport } from "../lib/exporting";
import { useApp } from "../store/app";
import { Button, Spinner } from "./ui";

export function ShareCardModal({ runId, modelKey, onClose }: { runId: string; modelKey: string; onClose: () => void }) {
  const [svg, setSvg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const notify = useApp((s) => s.notify);

  useEffect(() => {
    api.exportRun(runId, "svg", modelKey).then(setSvg).catch((e) => setError(String(e)));
  }, [runId, modelKey]);

  async function run(fn: () => Promise<string | null>) {
    try {
      const where = await fn();
      if (where) notify("ok", `Saved ${where}`);
    } catch (e) {
      notify("error", e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-6" role="dialog" aria-modal aria-label="Share card" onClick={onClose}>
      <div className="w-full max-w-[860px] rounded-md border border-line2 bg-panel p-4" onClick={(e) => e.stopPropagation()}>
        <div className="mb-3 flex items-center justify-between">
          <h2 className="label">Share card</h2>
          <Button variant="ghost" onClick={onClose}>Close</Button>
        </div>
        {error ? <p className="text-bad">{error}</p> : !svg ? <Spinner label="Rendering…" /> : (
          <img alt="PulseBench share card" className="w-full rounded-sm border border-line" src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`} />
        )}
        <p className="mt-2 text-[11.5px] text-dim">Generated locally. No account, no upload.</p>
        <div className="mt-3 flex gap-2">
          <Button variant="primary" disabled={!svg} onClick={() => void run(() => savePng(runId, modelKey, svg!))}>Save PNG</Button>
          <Button disabled={!svg} onClick={() => void run(() => saveExport(runId, "svg", modelKey))}>Save SVG</Button>
        </div>
      </div>
    </div>
  );
}
