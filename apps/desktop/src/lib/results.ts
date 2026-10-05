import type { ComponentScores, ModelResult, RunResult } from "@pulsebench/types";

export type SortKey = "score" | "correctness" | "passRate" | "tokens" | "vram" | "memory" | "runtime" | "latency";

export const sortLabels: Record<SortKey, string> = {
  score: "Score", correctness: "Correctness", passRate: "Pass rate", tokens: "Tokens/s", vram: "VRAM", memory: "Memory", runtime: "Total time", latency: "Avg generation",
};

/** Value used for sorting (higher is better unless `lowerIsBetter`). `null` always sorts last. */
export function sortValue(m: ModelResult, key: SortKey): number | null {
  switch (key) {
    case "score": return m.score?.pulsebenchScore ?? null;
    case "correctness": return m.score?.components.correctness ?? null;
    case "passRate": return m.stats.tasksTotal - m.stats.tasksSkipped > 0 ? m.stats.passRate : null;
    case "tokens": return m.stats.avgTokensPerSecond;
    case "vram": return m.stats.resources.peakVramMb;
    case "memory": return m.stats.resources.peakRamMb;
    case "runtime": return m.stats.totalSeconds;
    case "latency": return m.stats.avgGenerationSeconds;
  }
}

export const lowerIsBetter: ReadonlySet<SortKey> = new Set(["vram", "memory", "runtime", "latency"]);

export function sortModels(models: ModelResult[], key: SortKey, desc: boolean): ModelResult[] {
  const dir = desc ? -1 : 1;
  return [...models].sort((a, b) => {
    const va = sortValue(a, key);
    const vb = sortValue(b, key);
    if (va === null && vb === null) return 0;
    if (va === null) return 1;
    if (vb === null) return -1;
    return (va - vb) * dir;
  });
}

/** Default ranking: by PulseBench Score, then pass rate, then total time (unscored last). */
export function rankModels(models: ModelResult[]): ModelResult[] {
  return [...models].sort((a, b) => {
    const sa = a.score?.pulsebenchScore ?? -1;
    const sb = b.score?.pulsebenchScore ?? -1;
    return sb - sa || b.stats.passRate - a.stats.passRate || a.stats.totalSeconds - b.stats.totalSeconds;
  });
}

export const componentRows: Array<{ key: keyof ComponentScores; label: string; weightKey: keyof RunResult["settings"]["scoring"]["weights"]; help: string }> = [
  { key: "correctness", label: "Correctness", weightKey: "correctness", help: "Tasks fully solved (all verification passed)" },
  { key: "tests", label: "Test success", weightKey: "tests", help: "Share of tests passing, with partial credit" },
  { key: "compile", label: "Compilation", weightKey: "compile", help: "Compile / type-check succeeded (tasks that have such a step)" },
  { key: "efficiency", label: "Efficiency", weightKey: "efficiency", help: "Solved on the first attempt scores highest" },
  { key: "speed", label: "Generation speed", weightKey: "speed", help: "Tokens/second on a log scale between the floor and ceiling" },
  { key: "reliability", label: "Reliability", weightKey: "reliability", help: "Clean structured output; recovered or failed output scores less" },
];

export function summarySentence(run: RunResult, ranked: ModelResult[]): string | null {
  const first = ranked[0];
  if (!first?.score) return null;
  const gpu = run.system.gpus[0]?.name ?? "hardware";
  const scored = first.stats.tasksTotal - first.stats.tasksSkipped;
  let s = `On your ${gpu}, ${first.model.displayName} completed ${first.stats.tasksPassed}/${scored} tasks, scored ${first.score.pulsebenchScore.toLocaleString("en-US")}`;
  if (first.stats.avgTokensPerSecond !== null) s += `, averaged ${first.stats.avgTokensPerSecond.toFixed(1)} tokens/sec`;
  const second = ranked[1];
  if (second?.score && second.score.pulsebenchScore > 0) {
    const d = ((first.score.pulsebenchScore - second.score.pulsebenchScore) / second.score.pulsebenchScore) * 100;
    s += ` and outperformed ${second.model.displayName} by ${d.toFixed(1)}%`;
  }
  return s + " on this benchmark.";
}
