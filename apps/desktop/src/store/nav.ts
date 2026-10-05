import { create } from "zustand";

export type Route =
  | { page: "dashboard" }
  | { page: "new"; preselect?: string[]; suiteId?: string }
  | { page: "live" }
  | { page: "models" }
  | { page: "suites" }
  | { page: "results"; runId?: string }
  | { page: "model"; runId: string; modelKey: string; taskId?: string }
  | { page: "compare"; runId?: string; modelKeys?: string[] }
  | { page: "hardware" }
  | { page: "history" }
  | { page: "settings" };

interface NavState {
  route: Route;
  go: (route: Route) => void;
}

export const useNav = create<NavState>((set) => ({
  route: { page: "dashboard" },
  go: (route) => set({ route }),
}));

export const navItems: Array<{ page: Route["page"]; label: string; route: Route }> = [
  { page: "dashboard", label: "Dashboard", route: { page: "dashboard" } },
  { page: "new", label: "New Benchmark", route: { page: "new" } },
  { page: "models", label: "Models", route: { page: "models" } },
  { page: "suites", label: "Suites", route: { page: "suites" } },
  { page: "results", label: "Results", route: { page: "results" } },
  { page: "compare", label: "Compare", route: { page: "compare" } },
  { page: "hardware", label: "Hardware", route: { page: "hardware" } },
  { page: "history", label: "History", route: { page: "history" } },
  { page: "settings", label: "Settings", route: { page: "settings" } },
];
