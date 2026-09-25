import { describe, expect, it } from "vitest";
import { windowRange } from "./virtual";

describe("windowRange", () => {
  it("starts at the top with a full window", () => {
    const range = windowRange(1000, 0, 360, 36);
    expect(range).toEqual({
      first: 0,
      last: 26,
      padTop: 0,
      padBottom: (1000 - 26) * 36,
    });
  });

  it("scrolls the window with overscan", () => {
    const range = windowRange(1000, 3600, 360, 36);
    expect(range.first).toBe(92);
    expect(range.last).toBe(118);
    expect(range.padTop).toBe(92 * 36);
    expect(range.padBottom).toBe((1000 - 118) * 36);
  });

  it("clamps to the end so a short list keeps its rows visible", () => {
    const range = windowRange(8, 10_000, 360, 36);
    expect(range.first).toBe(0);
    expect(range.last).toBe(8);
    expect(range.padBottom).toBe(0);
  });

  it("clamps a shrunk list back into view", () => {
    const range = windowRange(30, 3600, 360, 36);
    expect(range.first).toBe(4);
    expect(range.last).toBe(30);
  });

  it("handles an empty list", () => {
    expect(windowRange(0, 0, 360, 36)).toEqual({
      first: 0,
      last: 0,
      padTop: 0,
      padBottom: 0,
    });
  });

  it("renders at least one row before the viewport is measured", () => {
    expect(windowRange(100, 0, 0, 36).last).toBeGreaterThan(0);
  });

  it("keeps spacers plus visible rows equal to the content height", () => {
    const total = 1000;
    const rowHeight = 36;
    const range = windowRange(total, 1234, 360, rowHeight);
    expect(
      range.padTop + (range.last - range.first) * rowHeight + range.padBottom,
    ).toBe(total * rowHeight);
  });
});
