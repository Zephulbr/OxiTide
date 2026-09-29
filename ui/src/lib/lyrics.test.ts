import { describe, expect, it } from "vitest";
import type { LyricLine } from "./api";
import { activeLine } from "./lyrics";

const sync = (...times: number[]): LyricLine[] =>
  times.map((time, index) => ({ time_ms: time, text: `line ${index}` }));

describe("activeLine", () => {
  const lines = sync(1000, 5000, 9000);

  it("is -1 before the first line", () => {
    expect(activeLine(lines, 0)).toBe(-1);
  });

  it("picks the latest line at or before the threshold", () => {
    expect(activeLine(lines, 6000)).toBe(1);
  });

  it("is the last line after the lyrics end", () => {
    expect(activeLine(lines, 20_000)).toBe(2);
  });

  it("counts a line exactly at its timestamp", () => {
    expect(activeLine(lines, 5000, 0)).toBe(1);
  });

  it("applies the lookahead window", () => {
    expect(activeLine(lines, 4900)).toBe(1);
    expect(activeLine(lines, 4800)).toBe(0);
  });

  it("skips blank timed lines", () => {
    const spaced: LyricLine[] = [
      { time_ms: 0, text: "a" },
      { time_ms: 1000, text: "   " },
    ];
    expect(activeLine(spaced, 2000)).toBe(0);
  });

  it("returns -1 when there are no timed lines", () => {
    expect(activeLine([{ time_ms: null, text: "plain" }], 10_000)).toBe(-1);
    expect(activeLine([], 10_000)).toBe(-1);
  });

  it("tolerates out-of-order timestamps", () => {
    const shuffled: LyricLine[] = [sync(5000)[0], sync(1000)[0]];
    expect(activeLine(shuffled, 6000)).toBe(0);
  });
});
