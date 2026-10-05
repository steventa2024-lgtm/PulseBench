import { getTransport } from "../ipc/client";

/** Native folder picker in the desktop app; a path prompt in the headless bridge (dev/test only). */
export async function pickFolder(): Promise<string | null> {
  const t = await getTransport();
  if (t.kind === "tauri") {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const r = await open({ directory: true, multiple: false });
    return typeof r === "string" ? r : null;
  }
  return window.prompt("Path of the folder to import") || null;
}

export async function pickSavePath(defaultName: string): Promise<string | null> {
  const t = await getTransport();
  if (t.kind === "tauri") {
    const { save } = await import("@tauri-apps/plugin-dialog");
    return (await save({ defaultPath: defaultName })) ?? null;
  }
  return window.prompt("Save to path", defaultName) || null;
}
