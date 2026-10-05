import { useState } from "react";
import type { SuiteSummary } from "@pulsebench/types";
import { Badge, Button, PageHeader, Panel } from "../components/ui";
import { api } from "../ipc/commands";
import { pickFolder } from "../lib/dialogs";
import { cx } from "../lib/format";
import { useApp } from "../store/app";
import { useNav } from "../store/nav";

export function Suites() {
  const { suites, refreshSuites, notify } = useApp();
  const go = useNav((s) => s.go);
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  async function importFolder() {
    try {
      const path = await pickFolder();
      if (!path) return;
      const s = await api.importSuite(path);
      await refreshSuites();
      notify("ok", `Imported ${s.name} (${s.taskCount} tasks)`);
    } catch (e) {
      notify("error", e instanceof Error ? e.message : String(e));
    }
  }

  async function prepare(id: string) {
    setBusy(id);
    try {
      const r = await api.prepareSuite(id);
      notify(r.ready ? "ok" : "error", r.message);
    } catch (e) {
      notify("error", String(e));
    } finally {
      setBusy(null);
    }
  }

  async function remove(s: SuiteSummary) {
    try {
      await api.removeCustomSuite(s.id);
      await refreshSuites();
    } catch (e) {
      notify("error", String(e));
    }
  }

  return (
    <div className="mx-auto max-w-[1100px]">
      <PageHeader title="Suites" sub="Versioned collections of runnable tasks. Changing a task requires a new suite version." actions={<Button onClick={() => void importFolder()}>Import custom suite…</Button>} />
      <div className="flex flex-col gap-3">
        {suites?.suites.map((s) => (
          <Panel key={s.id} pad={false}>
            <div className="flex items-start justify-between gap-4 p-3.5">
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-[14px] font-semibold">{s.name}</span><span className="num text-dim">v{s.version}</span>
                  {s.official ? <Badge kind="accent">official · lock verified</Badge> : s.lockVerified === false ? <Badge kind="bad" title="Content differs from suite.lock.json">modified without version bump</Badge> : <Badge kind="warn">custom</Badge>}
                  <Badge>{s.taskCount} tasks</Badge>
                </div>
                <p className="mt-1 text-muted">{s.description}</p>
                <p className="num mt-1 truncate text-[11px] text-dim" title={s.contentHash}>content hash {s.contentHash.slice(0, 24)}… · requires {s.requires.join(", ")}</p>
              </div>
              <div className="flex shrink-0 gap-2">
                <Button variant="ghost" onClick={() => setOpen(open === s.id ? null : s.id)}>{open === s.id ? "Hide tasks" : "Tasks"}</Button>
                {s.requires.includes("node") && <Button variant="ghost" disabled={busy === s.id} onClick={() => void prepare(s.id)}>{busy === s.id ? "Installing…" : "Prepare toolchain"}</Button>}
                <Button variant="primary" onClick={() => go({ page: "new", suiteId: s.id })}>Use</Button>
                {!s.official && <Button variant="danger" onClick={() => void remove(s)}>Remove</Button>}
              </div>
            </div>
            {open === s.id && (
              <table className="w-full border-t border-line text-[12px]">
                <tbody>
                  {s.tasks.map((t) => (
                    <tr key={t.id} className="border-b border-line/50 last:border-0">
                      <td className="num px-3.5 py-1.5 text-dim">{t.id}</td>
                      <td className="py-1.5">{t.title}</td>
                      <td className="py-1.5 text-muted">{t.category}</td>
                      <td className="py-1.5 text-muted">{t.language}</td>
                      <td className={cx("px-3.5 py-1.5 text-right", t.difficulty === "hard" ? "text-bad" : t.difficulty === "medium" ? "text-warn" : "text-ok")}>{t.difficulty}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Panel>
        ))}
        {suites?.problems.map((p) => <Panel key={p.path}><p className="text-bad">Invalid suite at {p.path}</p><p className="text-[12px] text-muted">{p.error}</p></Panel>)}
      </div>
      <Panel className="mt-4" title="Creating your own suite">
        <p className="text-[12px] text-muted">A suite is a folder with <span className="num">suite.json</span> and <span className="num">tasks/&lt;id&gt;/task.json</span> plus a runnable <span className="num">workspace/</span> fixture. Check it with <span className="num text-text">pulsebench suite validate &lt;folder&gt;</span> (the untouched fixture must fail and the reference <span className="num">solution/</span> must pass) before importing. Custom suites execute their test commands on your machine: only import suites you trust.</p>
      </Panel>
    </div>
  );
}
