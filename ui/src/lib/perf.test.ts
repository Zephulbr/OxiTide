import { describe, expect, it } from "vitest";
import { FrameSampler, LongTaskLog, formatHudLines, percentile } from "./perf";

describe("percentile", () => {
  it("returns the nearest-rank percentile", () => {
    expect(percentile([5, 1, 3, 2, 4], 50)).toBe(3);
    expect(percentile([1, 2, 3, 4], 100)).toBe(4);
    expect(percentile([1, 2, 3, 4], 1)).toBe(1);
  });

  it("returns 0 for an empty sample", () => {
    expect(percentile([], 95)).toBe(0);
  });
});

describe("FrameSampler", () => {
  it("summarizes recorded frame times", () => {
    const sampler = new FrameSampler(3);
    sampler.record(10);
    sampler.record(20);
    sampler.record(30);
    sampler.record(40);
    const summary = sampler.summary();
    expect(summary.count).toBe(3);
    expect(summary.max).toBe(40);
    expect(summary.p95).toBe(40);
  });

  it("is empty before any frames", () => {
    expect(new FrameSampler().summary()).toEqual({ count: 0, p50: 0, p95: 0, p99: 0, max: 0 });
  });
});

describe("LongTaskLog", () => {
  it("counts and tracks the maximum", () => {
    const log = new LongTaskLog();
    log.record(60);
    log.record(120);
    expect(log.summary()).toEqual({ count: 2, max_ms: 120 });
  });
});

describe("formatHudLines", () => {
  it("renders frame and long-task lines", () => {
    const lines = formatHudLines(
      { run_id: "S5-instrument-1", scenario: "S5-instrument", memory_mb: 250, last_ttfs_ms: 420, cache_hits: 3, cache_misses: 1 },
      { count: 10, p50: 4, p95: 12, p99: 30, max: 44 },
      { count: 1, max_ms: 60 },
    );
    expect(lines[0]).toContain("S5-instrument");
    expect(lines.join("\n")).toContain("p95 12.0");
    expect(lines.join("\n")).toContain("memory 250 MB");
    expect(lines.join("\n")).toContain("TTFS 420 ms");
  });
});
