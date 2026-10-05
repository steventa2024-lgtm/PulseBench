import type { GenerationProgress, LogEntry, Phase, RunEvent, RunStatus, TaskStatus, TelemetrySample } from "@pulsebench/types";

export interface LiveModel {
  key: string;
  displayName: string;
  tasks: Array<{ id: string; status: TaskStatus; score: number | null }>;
  score: number | null;
  done: boolean;
}

export interface LiveRun {
  runId: string;
  suiteName: string;
  tasksPerModel: number;
  models: LiveModel[];
  modelOrder: string[];
  currentModel: string | null;
  modelIndex: number;
  modelCount: number;
  taskIndex: number;
  taskTitle: string | null;
  taskId: string | null;
  phase: Phase | null;
  attempt: number;
  generation: GenerationProgress | null;
  taskStartedAt: number | null;
  logs: LogEntry[];
  telemetry: TelemetrySample[];
  status: RunStatus | "starting";
  finished: boolean;
}

const MAX_LOGS = 400;
const MAX_TELEMETRY = 600;

export function emptyLive(runId: string): LiveRun {
  return {
    runId, suiteName: "", tasksPerModel: 0, models: [], modelOrder: [], currentModel: null, modelIndex: 0, modelCount: 0,
    taskIndex: 0, taskTitle: null, taskId: null, phase: null, attempt: 0, generation: null, taskStartedAt: null,
    logs: [], telemetry: [], status: "starting", finished: false,
  };
}

/** Pure reducer: folds a backend event into the live-run view model. */
export function applyEvent(state: LiveRun | null, ev: RunEvent, now = Date.now()): LiveRun | null {
  if (ev.type === "runStarted") {
    return {
      ...emptyLive(ev.runId),
      logs: state && state.runId === ev.runId ? state.logs : [],
      suiteName: ev.suiteName,
      tasksPerModel: ev.tasksPerModel,
      modelCount: ev.models.length,
      status: "running",
    };
  }
  if (!state || ("runId" in ev && ev.runId !== state.runId)) return state;
  switch (ev.type) {
    case "modelStarted":
      return {
        ...state,
        currentModel: ev.modelKey,
        modelIndex: ev.index,
        modelCount: ev.total,
        taskIndex: 0,
        taskTitle: null,
        phase: null,
        generation: null,
        models: state.models.some((m) => m.key === ev.modelKey)
          ? state.models
          : [...state.models, { key: ev.modelKey, displayName: ev.displayName, tasks: [], score: null, done: false }],
      };
    case "taskStarted":
      return { ...state, taskIndex: ev.index, taskTitle: ev.title, taskId: ev.taskId, generation: null, attempt: 1, taskStartedAt: now, phase: "preparing" };
    case "phaseChanged":
      return { ...state, phase: ev.phase, attempt: ev.attempt || state.attempt, generation: ev.phase === "generating" ? null : state.generation };
    case "generation":
      return { ...state, generation: ev.progress };
    case "telemetry":
      return { ...state, telemetry: [...state.telemetry, ev.sample].slice(-MAX_TELEMETRY) };
    case "log":
      return { ...state, logs: [...state.logs, ev.entry].slice(-MAX_LOGS) };
    case "taskFinished":
      return {
        ...state,
        models: state.models.map((m) => (m.key === ev.modelKey ? { ...m, tasks: [...m.tasks, { id: ev.taskId, status: ev.status, score: ev.score }] } : m)),
      };
    case "modelFinished":
      return { ...state, models: state.models.map((m) => (m.key === ev.modelKey ? { ...m, score: ev.score, done: true } : m)) };
    case "runFinished":
      return { ...state, status: ev.status, finished: true, phase: null, generation: null };
    default:
      return state;
  }
}

/** Overall progress `0..1` across all models. */
export function progressOf(run: LiveRun): number {
  const total = run.tasksPerModel * Math.max(run.modelCount, 1);
  if (total === 0) return 0;
  const done = run.models.reduce((n, m) => n + m.tasks.length, 0);
  return Math.min(1, done / total);
}

export const phaseLabel: Record<Phase, string> = {
  preparing: "PREPARING",
  "loading-model": "LOADING MODEL",
  generating: "GENERATING",
  parsing: "PARSING RESPONSE",
  applying: "APPLYING PATCH",
  compiling: "COMPILING",
  testing: "RUNNING TESTS",
  verifying: "VERIFYING",
  scoring: "SCORING",
};
