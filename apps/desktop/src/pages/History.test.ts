import { describe, expect, it } from "vitest";
import type { HistoryPoint } from "@pulsebench/types";
import { deltas } from "./History";

const p = (score: number | null, over: Partial<HistoryPoint> = {}): HistoryPoint => ({
  runId: String(score), createdAt: "2026-01-01T00:00:00Z", suiteId: "quick", suiteVersion: "1.0.0", modelKey: "a/b", displayName: "b",
  score, passRate: 0.5, avgTokensPerSecond: null, gpu: null, standardSettings: true, ...over,
});

describe("history deltas", () => {
  it("computes relative change between consecutive runs", () => {
    const d = deltas([p(8390), p(8742)]);
    expect(d[0]).toEqual({ change: null, comparable: true });
    expect(d[1]!.change).toBeCloseTo(4.2, 1);
    expect(d[1]!.comparable).toBe(true);
  });
  it("flags suite-version and settings changes as not comparable", () => {
    expect(deltas([p(100), p(110, { suiteVersion: "2.0.0" })])[1]!.comparable).toBe(false);
    expect(deltas([p(100), p(110, { standardSettings: false })])[1]!.comparable).toBe(false);
  });
});
