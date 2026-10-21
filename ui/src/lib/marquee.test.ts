import { describe, expect, it } from "vitest";
import { MARQUEE_HOLD_MS, MARQUEE_SPEED_PX_S, marqueeTiming } from "./marquee";

describe("marqueeTiming", () => {
  it("travels at a fixed reading speed with a hold at each end", () => {
    const { duration, offsets } = marqueeTiming(60);
    const travel = (60 / MARQUEE_SPEED_PX_S) * 1000;
    expect(duration).toBe(2 * (MARQUEE_HOLD_MS + travel));
    expect(offsets[0] * duration).toBeCloseTo(MARQUEE_HOLD_MS);
    expect((offsets[1] - offsets[0]) * duration).toBeCloseTo(travel);
    expect((offsets[2] - offsets[1]) * duration).toBeCloseTo(MARQUEE_HOLD_MS);
    expect((1 - offsets[2]) * duration).toBeCloseTo(travel);
  });

  it("takes longer for longer overflow", () => {
    expect(marqueeTiming(200).duration).toBeGreaterThan(marqueeTiming(20).duration);
  });
});
