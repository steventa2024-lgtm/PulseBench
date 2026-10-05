import { describe, expect, it } from "vitest";
import type { RunEvent } from "@pulsebench/types";
import { applyEvent, progressOf, type LiveRun } from "./live";

const feed = (events: RunEvent[]): LiveRun | null => events.reduce<LiveRun | null>((s, e) => applyEvent(s, e, 1000), null);

describe("live run reducer", () => {
  const start: RunEvent = { type: "runStarted", runId: "r1", suiteName: "Quick Coding", models: ["a", "b"], tasksPerModel: 2 };

  it("tracks models, tasks and overall progress", () => {
    const s = feed([
      start,
      { type: "modelStarted", runId: "r1", modelKey: "ollama/a", displayName: "a", index: 0, total: 2 },
      { type: "taskStarted", runId: "r1", modelKey: "ollama/a", taskId: "t1", title: "Fix bug", index: 0, total: 2 },
      { type: "phaseChanged", runId: "r1", modelKey: "ollama/a", taskId: "t1", phase: "generating", attempt: 1 },
      { type: "taskFinished", runId: "r1", modelKey: "ollama/a", taskId: "t1", status: "passed", score: 97.5, index: 0, total: 2 },
    ])!;
    expect(s.models[0]!.tasks).toEqual([{ id: "t1", status: "passed", score: 97.5 }]);
    expect(s.phase).toBe("generating");
    expect(progressOf(s)).toBeCloseTo(0.25);
  });

  it("ignores events from other runs", () => {
    const s = feed([start, { type: "log", runId: "other", entry: { ts: "2026-01-01T00:00:00Z", level: "info", scope: "X", message: "m" } }])!;
    expect(s.logs).toHaveLength(0);
  });

  it("caps logs and marks the run finished", () => {
    let s = applyEvent(null, start)!;
    for (let i = 0; i < 500; i++) {
      s = applyEvent(s, { type: "log", runId: "r1", entry: { ts: "2026-01-01T00:00:00Z", level: "info", scope: "X", message: String(i) } })!;
    }
    expect(s.logs).toHaveLength(400);
    expect(s.logs.at(-1)!.message).toBe("499");
    s = applyEvent(s, { type: "runFinished", runId: "r1", status: "cancelled" })!;
    expect(s.finished).toBe(true);
    expect(s.status).toBe("cancelled");
  });
});
