import { create } from "zustand";
import type { AppSettings, DataInfo, LatestScore, ProviderStatus, RunSummary, SuiteList, SystemInfo } from "@pulsebench/types";
import { api } from "../ipc/commands";
import { getTransport } from "../ipc/client";
import { applyEvent, emptyLive, type LiveRun } from "./live";
import { useNav } from "./nav";

interface AppState {
  ready: boolean;
  fatal: string | null;
  transport: "tauri" | "bridge" | null;
  data: DataInfo | null;
  system: SystemInfo | null;
  providers: ProviderStatus[] | null;
  providersLoading: boolean;
  suites: SuiteList | null;
  settings: AppSettings | null;
  runs: RunSummary[];
  latest: LatestScore[];
  live: LiveRun | null;
  toast: { kind: "ok" | "error"; text: string } | null;
  init: () => Promise<void>;
  refreshProviders: () => Promise<void>;
  refreshRuns: () => Promise<void>;
  refreshSuites: () => Promise<void>;
  refreshSettings: () => Promise<void>;
  setSettings: (s: AppSettings) => void;
  notify: (kind: "ok" | "error", text: string) => void;
  startLive: (runId: string) => void;
}

let toastTimer: ReturnType<typeof setTimeout> | undefined;
let initStarted = false;

export const useApp = create<AppState>((set, get) => ({
  ready: false,
  fatal: null,
  transport: null,
  data: null,
  system: null,
  providers: null,
  providersLoading: false,
  suites: null,
  settings: null,
  runs: [],
  latest: [],
  live: null,
  toast: null,

  async init() {
    // React StrictMode runs effects twice in development; subscribing twice would duplicate every event.
    if (initStarted) return;
    initStarted = true;
    try {
      const t = await getTransport();
      set({ transport: t.kind });
      const [data, settings, suites, runs, latest, active] = await Promise.all([
        api.dataInfo(), api.settings(), api.suites(), api.listRuns(), api.latestScores(), api.activeRun(),
      ]);
      set({ data, settings, suites, runs, latest, ready: true });
      await t.subscribe((ev) => {
        set((s) => ({ live: applyEvent(s.live, ev) }));
        if (ev.type === "runFinished") {
          void get().refreshRuns();
        }
      });
      // Non-blocking: hardware and provider discovery must never delay the first paint.
      void api.systemInfo().then((system) => set({ system })).catch((e) => get().notify("error", String(e)));
      void get().refreshProviders();
      if (active) {
        get().startLive(active.runId);
        useNav.getState().go({ page: "live" });
      }
    } catch (e) {
      set({ fatal: e instanceof Error ? e.message : String(e), ready: true });
    }
  },

  async refreshProviders() {
    set({ providersLoading: true });
    try {
      set({ providers: await api.providerStatuses() });
    } catch (e) {
      get().notify("error", String(e));
    } finally {
      set({ providersLoading: false });
    }
  },
  async refreshRuns() {
    const [runs, latest] = await Promise.all([api.listRuns(), api.latestScores()]);
    set({ runs, latest });
  },
  async refreshSuites() {
    set({ suites: await api.suites() });
  },
  async refreshSettings() {
    set({ settings: await api.settings() });
  },
  setSettings: (settings) => set({ settings }),
  notify(kind, text) {
    clearTimeout(toastTimer);
    set({ toast: { kind, text } });
    toastTimer = setTimeout(() => set({ toast: null }), kind === "error" ? 8000 : 3500);
  },
  startLive: (runId) => set({ live: emptyLive(runId) }),
}));
