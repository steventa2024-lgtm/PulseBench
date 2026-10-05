import { describe, expect, it } from "vitest";
import { bytes, cx, mb, ms, pct, percentChange, secs, signedPct, thousands } from "./format";

describe("format", () => {
  it("formats scores with thousands separators", () => {
    expect(thousands(8742)).toBe("8,742");
    expect(thousands(10000)).toBe("10,000");
  });
  it("never invents missing values", () => {
    expect(pct(null)).toBe("—");
    expect(mb(null)).toBe("unavailable");
    expect(ms(null)).toBe("n/a");
    expect(bytes(undefined)).toBe("—");
  });
  it("formats sizes and durations", () => {
    expect(bytes(18_500_000_000)).toBe("18.5 GB");
    expect(mb(8192)).toBe("8.0 GB");
    expect(secs(125)).toBe("2m 05s");
    expect(secs(3725)).toBe("1h 02m");
    expect(secs(4.25)).toBe("4.3s");
    expect(ms(1500)).toBe("1.5s");
  });
  it("computes history changes", () => {
    expect(signedPct(percentChange(8742, 8390))).toBe("+4.2%");
    expect(signedPct(percentChange(100, 0))).toBe("—");
  });
  it("joins class names", () => {
    expect(cx("a", false, null, "b")).toBe("a b");
  });
});
