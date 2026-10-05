// Typed wrappers for every backend command. The names and shapes mirror crates/app/src/rpc.rs.
import type {
  ActiveRunInfo, AppSettings, DataInfo, ExportFormat, HistoryPoint, LatestScore, PrepareReport, ProviderStatus,
  RunResult, RunSettings, RunSummary, StartRunRequest, SuiteList, SuiteSummary, SystemInfo, TelemetrySample,
} from "@pulsebench/types";
import { call } from "./client";

export const api = {
  dataInfo: () => call<DataInfo>("get_data_info"),
  systemInfo: (refresh = false) => call<SystemInfo>("get_system_info", { refresh }),
  liveSample: () => call<TelemetrySample>("live_sample"),
  defaultRunSettings: () => call<RunSettings>("default_run_settings"),
  settings: () => call<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) => call<AppSettings>("save_settings", { settings }),
  markOnboardingDone: () => call<null>("mark_onboarding_done"),
  providerStatuses: () => call<ProviderStatus[]>("provider_statuses"),
  suites: () => call<SuiteList>("list_suites"),
  importSuite: (path: string) => call<SuiteSummary>("import_suite", { path }),
  removeCustomSuite: (id: string) => call<null>("remove_custom_suite", { id }),
  prepareSuite: (suiteId: string) => call<PrepareReport>("prepare_suite", { suiteId }),
  startRun: (request: StartRunRequest) => call<string>("start_run", { request }),
  cancelRun: () => call<boolean>("cancel_run"),
  activeRun: () => call<ActiveRunInfo | null>("active_run"),
  listRuns: (limit = 100) => call<RunSummary[]>("list_runs", { limit }),
  getRun: (id: string) => call<RunResult>("get_run", { id }),
  deleteRun: (id: string) => call<null>("delete_run", { id }),
  clearHistory: () => call<number>("clear_history"),
  modelHistory: (modelKey: string, suiteId?: string) => call<HistoryPoint[]>("model_history", { modelKey, suiteId: suiteId ?? null }),
  latestScores: () => call<LatestScore[]>("latest_scores"),
  exportRun: (id: string, format: ExportFormat, modelKey?: string) => call<string>("export_run", { id, format, modelKey: modelKey ?? null }),
  exportRunToFile: (id: string, format: ExportFormat, path: string, modelKey?: string) =>
    call<null>("export_run_to_file", { id, format, modelKey: modelKey ?? null, path }),
  exportDatabase: (path: string) => call<null>("export_database", { path }),
  savePng: (path: string, base64: string) => call<null>("save_png", { path, base64 }),
};
