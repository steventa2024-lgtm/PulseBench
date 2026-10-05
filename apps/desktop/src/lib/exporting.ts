import type { ExportFormat } from "@pulsebench/types";
import { api } from "../ipc/commands";
import { getTransport } from "../ipc/client";

const filters: Record<ExportFormat, { name: string; extensions: string[] }> = {
  json: { name: "PulseBench result (JSON)", extensions: ["json"] },
  markdown: { name: "Markdown report", extensions: ["md"] },
  svg: { name: "SVG share card", extensions: ["svg"] },
};
const ext: Record<ExportFormat, string> = { json: "json", markdown: "md", svg: "svg" };

function download(name: string, blob: Blob) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** Save an export: a native "Save as" dialog in the desktop app, a browser download otherwise. Returns where it went. */
export async function saveExport(runId: string, format: ExportFormat, modelKey?: string): Promise<string | null> {
  const name = `pulsebench-${runId}${modelKey ? `-${modelKey.replace(/[^a-z0-9]+/gi, "_")}` : ""}.${ext[format]}`;
  const t = await getTransport();
  if (t.kind === "tauri") {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({ defaultPath: name, filters: [filters[format]] });
    if (!path) return null;
    await api.exportRunToFile(runId, format, path, modelKey);
    return path;
  }
  const text = await api.exportRun(runId, format, modelKey);
  const mime = format === "json" ? "application/json" : format === "svg" ? "image/svg+xml" : "text/markdown";
  download(name, new Blob([text], { type: mime }));
  return name;
}

export async function svgToPngBase64(svg: string, width = 1200, height = 630): Promise<string> {
  const img = new Image();
  const url = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  await new Promise<void>((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () => reject(new Error("could not render the card"));
    img.src = url;
  });
  const canvas = document.createElement("canvas");
  canvas.width = width * 2;
  canvas.height = height * 2;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("canvas unavailable");
  ctx.scale(2, 2);
  ctx.drawImage(img, 0, 0, width, height);
  return canvas.toDataURL("image/png").split(",")[1] ?? "";
}

export async function savePng(runId: string, modelKey: string, svg: string): Promise<string | null> {
  const name = `pulsebench-${runId}-${modelKey.replace(/[^a-z0-9]+/gi, "_")}.png`;
  const b64 = await svgToPngBase64(svg);
  const t = await getTransport();
  if (t.kind === "tauri") {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({ defaultPath: name, filters: [{ name: "PNG image", extensions: ["png"] }] });
    if (!path) return null;
    await api.savePng(path, b64);
    return path;
  }
  const bin = atob(b64);
  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
  download(name, new Blob([bytes], { type: "image/png" }));
  return name;
}
