import { describe, expect, it } from "vitest";
import type { LyricLine } from "./api";
import { activeLine, lineWords } from "./lyrics";

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

describe("lineWords", () => {
  const line = (time_ms: number | null, text: string, words?: LyricLine["words"]): LyricLine => ({
    time_ms,
    text,
    words,
  });

  it("passes enhanced word stamps through verbatim", () => {
    const words = [
      { time_ms: 1000, text: "Hello " },
      { time_ms: 1500, text: "world" },
    ];
    expect(lineWords(line(1000, "Hello world", words), 3000)).toEqual(words);
  });

  it("estimates by word length across the gap to the next line", () => {
    // weights 1/2/4 over a 1400ms gap → 200ms per character.
    const words = lineWords(line(0, "I am here"), 1400);
    expect(words).toEqual([
      { time_ms: 0, text: "I " },
      { time_ms: 200, text: "am " },
      { time_ms: 600, text: "here" },
    ]);
  });

  it("preserves spacing tokens for rendering", () => {
    const words = lineWords(line(0, "a  b"), 1000);
    expect(words.map((word) => word.text)).toEqual(["a  ", "b"]);
  });

  it("falls back to a fixed pace for the last line", () => {
    expect(lineWords(line(1000, "one two"), null)).toEqual([
      { time_ms: 1000, text: "one " },
      { time_ms: 1400, text: "two" },
    ]);
  });

  it("keeps a packed line inside its gap so every word still flips", () => {
    const words = lineWords(line(0, "a b"), 100);
    expect(words).toEqual([
      { time_ms: 0, text: "a " },
      { time_ms: 50, text: "b" },
    ]);
  });

  it("falls back to a fixed pace when the next stamp is not after the line", () => {
    expect(lineWords(line(1000, "one two"), 500)).toEqual([
      { time_ms: 1000, text: "one " },
      { time_ms: 1400, text: "two" },
    ]);
  });

  it("clamps a sparse line to a maximum pace", () => {
    const words = lineWords(line(0, "a b"), 100_000);
    expect(words).toEqual([
      { time_ms: 0, text: "a " },
      { time_ms: 700, text: "b" },
    ]);
  });

  it("returns nothing for untimed or blank lines", () => {
    expect(lineWords(line(null, "plain"), 1000)).toEqual([]);
    expect(lineWords(line(0, "   "), 1000)).toEqual([]);
  });
});
