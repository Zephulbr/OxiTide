// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

/// `frameDue` reads the platform tag once, at import, and keeps its gate in
/// module state; each test gets a fresh copy.
async function load(platform?: string) {
  vi.resetModules();
  if (platform) document.documentElement.dataset.platform = platform;
  return (await import("./frameGate")).frameDue;
}

async function loadSpin(platform?: string) {
  vi.resetModules();
  if (platform) document.documentElement.dataset.platform = platform;
  return (await import("./frameGate")).gatedSpin;
}

/// A manual rAF: `run(t)` fires the callbacks queued for the frame at `t`.
function fakeFrames() {
  let queued: FrameRequestCallback[] = [];
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => queued.push(cb));
  vi.stubGlobal("cancelAnimationFrame", () => {
    queued = [];
  });
  return {
    run(t: number) {
      const due = queued;
      queued = [];
      due.forEach((cb) => cb(t));
    },
  };
}

describe("frameGate", () => {
  afterEach(() => {
    delete document.documentElement.dataset.platform;
    vi.unstubAllGlobals();
  });

  it("keeps each loop's own step off Linux", async () => {
    const frameDue = await load();
    expect(frameDue(16)).toBe(true);
    expect(frameDue(17)).toBe(true);
    expect(frameDue(100, null, 15)).toBe(true);
    expect(frameDue(110, 100, 15)).toBe(false);
    expect(frameDue(116, 100, 15)).toBe(true);
  });

  it("draws every other 60 Hz frame on Linux, with whole-ms frame times", async () => {
    const frameDue = await load("linux");
    const times = [1000, 1016, 1033, 1049, 1066, 1083, 1099, 1116];
    expect(times.map((t) => frameDue(t))).toEqual([true, false, true, false, true, false, true, false]);
  });

  it("opens one shared gate for every loop in a frame on Linux", async () => {
    const frameDue = await load("linux");
    // Two loops that started a frame apart still draw on the same frames.
    expect([frameDue(1000), frameDue(1000, 990)]).toEqual([true, true]);
    expect([frameDue(1016, 1000), frameDue(1016, null)]).toEqual([false, false]);
    expect([frameDue(1033, 1000), frameDue(1033, 1000, 1000 / 30 - 1)]).toEqual([true, true]);
  });

  it("leaves the spin to CSS off Linux", async () => {
    const gatedSpin = await loadSpin();
    const frames = fakeFrames();
    const node = document.createElement("span");
    gatedSpin(node, { spinning: true, turnMs: 1800 });
    frames.run(1000);
    frames.run(1033);
    expect(node.style.transform).toBe("");
  });

  it("turns on the gate's frames on Linux and returns to 0 when stopped", async () => {
    const gatedSpin = await loadSpin("linux");
    const frames = fakeFrames();
    const node = document.createElement("span");
    const action = gatedSpin(node, { spinning: true, turnMs: 1800 });
    frames.run(1000);
    expect(node.style.transform).toBe("rotate(0deg)");
    frames.run(1016);
    expect(node.style.transform).toBe("rotate(0deg)");
    frames.run(1450);
    expect(node.style.transform).toBe("rotate(90deg)");
    action.update?.({ spinning: false, turnMs: 1800 });
    expect(node.style.transform).toBe("");
  });
});
