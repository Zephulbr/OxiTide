import { describe, expect, it } from "vitest";
import type { BeatClock } from "./api";
import {
  BEAT_TRIM_MS,
  EASE_MS,
  JUMP_MS,
  SNAP_MS,
  beatIndex,
  beatPhase,
  createPositionClock,
} from "./beatClock";

const clock = (anchor_ms: number, period_ms = 500): BeatClock => ({
  track_id: 1,
  bpm: 60_000 / period_ms,
  period_ms,
  anchor_ms,
  confidence: 0.9,
});

describe("position clock", () => {
  it("knows nothing before the first sample", () => {
    expect(createPositionClock().heardAt(0)).toBeNull();
  });

  it("subtracts the output lead and extrapolates while playing", () => {
    const c = createPositionClock();
    c.setPlaying(true, 0);
    c.sample(10_000, 210, 1_000);
    expect(c.heardAt(1_000)).toBe(10_000 - 210 - BEAT_TRIM_MS);
    expect(c.heardAt(1_250)).toBe(10_040 - BEAT_TRIM_MS);
  });

  it("freezes while paused and resumes from where it stopped", () => {
    const c = createPositionClock();
    c.setPlaying(true, 0);
    c.sample(5_000, 0, 0);
    c.setPlaying(false, 300);
    expect(c.heardAt(300)).toBe(5_300);
    expect(c.heardAt(5_000)).toBe(5_300);
    c.setPlaying(true, 6_000);
    expect(c.heardAt(6_100)).toBe(5_400);
  });

  it("leaves a drift under the snap threshold alone", () => {
    const c = createPositionClock();
    c.setPlaying(true, 0);
    c.sample(1_000, 0, 0);
    c.sample(1_500 + SNAP_MS - 1, 0, 500);
    expect(c.heardAt(500)).toBe(1_500);
    expect(c.heardAt(600)).toBe(1_600);
  });

  it("eases a larger drift out over the ease time", () => {
    const c = createPositionClock();
    c.setPlaying(true, 0);
    c.sample(1_000, 0, 0);
    c.sample(1_600, 0, 500);
    expect(c.heardAt(500)).toBe(1_500);
    expect(c.heardAt(500 + EASE_MS / 2)).toBe(1_500 + EASE_MS / 2 + 50);
    expect(c.heardAt(500 + EASE_MS)).toBe(1_500 + EASE_MS + 100);
    expect(c.heardAt(500 + EASE_MS + 300)).toBe(1_500 + EASE_MS + 300 + 100);
  });

  it("snaps seeks and track changes at once", () => {
    const c = createPositionClock();
    c.setPlaying(true, 0);
    c.sample(1_000, 0, 0);
    c.sample(90_000, 0, 500, true);
    expect(c.heardAt(500)).toBe(90_000);
  });

  it("treats a drift past the jump threshold as a seek", () => {
    const c = createPositionClock();
    c.setPlaying(true, 0);
    c.sample(1_000, 0, 0);
    c.sample(1_500 + JUMP_MS, 0, 500);
    expect(c.heardAt(500)).toBe(1_500 + JUMP_MS);
  });
});

describe("beat math", () => {
  it("counts whole beats and the phase from the anchor", () => {
    const c = clock(10_000);
    expect(beatIndex(c, 10_000)).toBe(0);
    expect(beatPhase(c, 10_000)).toBe(0);
    expect(beatIndex(c, 10_750)).toBe(1);
    expect(beatPhase(c, 10_750)).toBeCloseTo(0.5);
  });

  it("wraps before the anchor", () => {
    const c = clock(10_000);
    expect(beatIndex(c, 9_750)).toBe(-1);
    expect(beatPhase(c, 9_750)).toBeCloseTo(0.5);
    expect(beatIndex(c, 9_000)).toBe(-2);
    expect(beatPhase(c, 9_000)).toBe(0);
  });

  it("is inert for a degenerate period", () => {
    const c = clock(0, 0);
    expect(beatIndex(c, 1_234)).toBe(0);
    expect(beatPhase(c, 1_234)).toBe(0);
  });
});
