// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import NowPlayingMark from "./NowPlayingMark.svelte";

vi.mock("../appearance.svelte", () => ({ appearance: { ambientMotion: true } }));

/// A manual frame clock: frames run only when the test steps them.
let queue: FrameRequestCallback[] = [];
let now = 0;
function step(frames: number) {
  for (let i = 0; i < frames && queue.length > 0; i++) {
    now += 16;
    const due = queue;
    queue = [];
    for (const callback of due) callback(now);
  }
}

beforeEach(() => {
  queue = [];
  now = 0;
  vi.spyOn(performance, "now").mockImplementation(() => now);
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    queue.push(callback);
    return queue.length;
  });
  vi.stubGlobal("cancelAnimationFrame", () => {
    queue = [];
  });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const angleOf = () =>
  Number(/rotate\(([-\d.]+)/.exec(document.querySelector("g")?.getAttribute("transform") ?? "")?.[1] ?? 0);

describe("NowPlayingMark", () => {
  it("runs no frame loop while paused at rest", () => {
    render(NowPlayingMark, { playing: false });
    expect(queue).toHaveLength(0);
  });

  it("spins while playing, eases to a stop on pause, then stops its loop", async () => {
    const { rerender } = render(NowPlayingMark, { playing: true });
    step(30);
    expect(queue).toHaveLength(1);
    const playingAngle = angleOf();
    expect(playingAngle).toBeGreaterThan(0);

    await rerender({ playing: false });
    step(1000);
    // Settled: no frame is pending, and the disc keeps the angle it stopped at.
    expect(queue).toHaveLength(0);
    const rest = angleOf();
    expect(rest).not.toBe(0);

    await rerender({ playing: true });
    expect(queue).toHaveLength(1);
    step(5);
    expect(angleOf()).not.toBe(rest);
  });
});
