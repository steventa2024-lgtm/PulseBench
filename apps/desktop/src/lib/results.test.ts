import { describe, expect, it } from "vitest";
import type { ModelResult } from "@pulsebench/types";
import { rankModels, sortModels, sortValue } from "./results";

function model(id: string, score: number | null, tps: number | null, total = 10): ModelResult {
  return {
    key: `p/${id}`,
    model: { providerId: "p", providerKind: "ollama", id, displayName: id, family: null, architecture: null, parameterSize: null, quantization: null, sizeBytes: null, contextLength: null, format: null, loaded: null, modifiedAt: null },
    status: "completed", error: null, residency: null,
    score: score === null ? null : { pulsebenchScore: score, components: { correctness: 0.5, tests: null, compile: null, efficiency: null, speed: null, reliability: null }, categories: [], formula: "x" },
    stats: { tasksTotal: 2, tasksPassed: 1, tasksFailed: 1, tasksSkipped: 0, tasksError: 0, passRate: 0.5, avgGenerationSeconds: 1, avgTokensPerSecond: tps, avgTimeToFirstTokenMs: null, totalPromptTokens: 0, totalCompletionTokens: 0, totalSeconds: total, repairAttempts: 0, compileFailures: 0, runtimeFailures: 0, protocolFailures: 0, resources: { samples: 0, peakCpuPercent: null, avgCpuPercent: null, peakRamMb: null, peakGpuPercent: null, peakVramMb: null, peakGpuTempC: null } },
    tasks: [],
  };
}

describe("results ranking", () => {
  const a = model("a", 7000, 30), b = model("b", 9000, 10), c = model("c", null, null);
  it("ranks by score with unscored last", () => {
    expect(rankModels([c, a, b]).map((m) => m.model.id)).toEqual(["b", "a", "c"]);
  });
  it("sorts by any column and keeps missing values last in both directions", () => {
    expect(sortModels([a, b, c], "tokens", true).map((m) => m.model.id)).toEqual(["a", "b", "c"]);
    expect(sortModels([a, b, c], "tokens", false).map((m) => m.model.id)).toEqual(["b", "a", "c"]);
  });
  it("exposes raw values without inventing any", () => {
    expect(sortValue(c, "score")).toBeNull();
    expect(sortValue(a, "vram")).toBeNull();
    expect(sortValue(a, "correctness")).toBe(0.5);
  });
});
