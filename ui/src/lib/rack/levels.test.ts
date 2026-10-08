import { describe, expect, it, vi } from "vitest";
import type { LevelFrame } from "../api";
import { createLevels } from "./levels.svelte";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("../api", () => ({ api: {}, LEVEL_FRAME_EVENT: "level_frame" }));

function harness(visible = true) {
  let handler: ((e: { payload: LevelFrame }) => void) | null = null;
  let frameCb: ((t: number) => void) | null = null;
  let onVis: (() => void) | null = null;
  const state = { visible };
  const deps = {
    listen: vi.fn(async (_e: string, h: (e: { payload: LevelFrame }) => void) => {
      handler = h;
      return () => (handler = null);
    }),
    setLevelMeter: vi.fn(async () => {}),
    raf: vi.fn((cb: (t: number) => void) => {
      frameCb = cb;
      return 1;
    }),
    cancelRaf: vi.fn(() => (frameCb = null)),
    visible: () => state.visible,
    onVisibility: (cb: () => void) => (onVis = cb),
  };
  const levels = createLevels(() => deps);
  return {
    levels,
    deps,
    state,
    emit: (f: LevelFrame) => handler?.({ payload: f }),
    tick: (t: number) => frameCb?.(t),
    visibilityChanged: () => onVis?.(),
  };
}

// RMS −20 / −26 dBFS = −2 / −8 VU: both on scale, left higher.
const loud: LevelFrame = { peak: [1, 0.5], rms: [0.1, 0.05], bands: Array(28).fill(0) };
const flush = () => new Promise((r) => setTimeout(r, 0));

describe("levels store", () => {
  it("enables the tap only while a meter is mounted", async () => {
    const h = harness();
    const release = h.levels.use();
    await flush();
    expect(h.deps.setLevelMeter).toHaveBeenCalledWith(true);
    release();
    expect(h.deps.setLevelMeter).toHaveBeenLastCalledWith(false);
    expect(h.deps.cancelRaf).toHaveBeenCalled();
  });

  it("stops while hidden and resumes when visible", async () => {
    const h = harness(false);
    h.levels.use();
    await flush();
    expect(h.deps.setLevelMeter).not.toHaveBeenCalled();
    h.state.visible = true;
    h.visibilityChanged();
    await flush();
    expect(h.deps.setLevelMeter).toHaveBeenCalledWith(true);
    h.state.visible = false;
    h.visibilityChanged();
    expect(h.deps.setLevelMeter).toHaveBeenLastCalledWith(false);
  });

  it("applies frames with ballistics", async () => {
    const h = harness();
    h.levels.use();
    await flush();
    h.tick(0);
    h.emit(loud);
    h.tick(16);
    expect(h.levels.snapshot.peakDb[0]).toBeCloseTo(0, 3);
    expect(h.levels.snapshot.peakDb[1]).toBeCloseTo(-6.02, 1);
    for (let t = 32; t < 600; t += 16) h.tick(t);
    expect(h.levels.snapshot.needle[0]).toBeGreaterThan(h.levels.snapshot.needle[1]);
    expect(h.levels.snapshot.holdDb[0]).toBeCloseTo(0, 3);
  });
});
