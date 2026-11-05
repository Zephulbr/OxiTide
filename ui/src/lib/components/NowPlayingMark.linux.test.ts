// @vitest-environment jsdom
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import type { Component } from "svelte";

vi.mock("../appearance.svelte", () => ({ appearance: { ambientMotion: true } }));

// `frameGate` reads the platform once, when it loads: set it first.
let NowPlayingMark: Component<{ playing: boolean }>;
beforeAll(async () => {
  document.documentElement.dataset.platform = "linux";
  NowPlayingMark = (await import("./NowPlayingMark.svelte")).default;
});

/// A manual frame clock: frames run only when the test steps them, 40 ms
/// apart so every one passes Linux's 30 fps gate.
let queue: FrameRequestCallback[] = [];
let now = 0;
function step(frames: number) {
  for (let i = 0; i < frames && queue.length > 0; i++) {
    now += 40;
    const due = queue;
    queue = [];
    for (const callback of due) callback(now);
  }
}

beforeEach(() => {
  // `now` never goes back: the gate keeps its last open time across tests.
  queue = [];
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

/// Turning, the wrapper carries the angle (a compositor rotation); at rest
/// the SVG does, with the wrapper untransformed.
const wrapperAngle = () => /rotate\(([-\d.]+)deg\)/.exec(document.querySelector<HTMLElement>(".np-mark")?.style.transform ?? "")?.[1];
const svgAngle = () => /rotate\(([-\d.]+)/.exec(document.querySelector("g")?.getAttribute("transform") ?? "")?.[1];
const angleOf = () => Number(wrapperAngle() ?? svgAngle() ?? 0);

describe("NowPlayingMark on Linux (gated loop)", () => {
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
    // Turning: the compositor rotates the wrapper; the SVG stays upright.
    expect(wrapperAngle()).toBeDefined();
    expect(Number(svgAngle())).toBe(0);

    await rerender({ playing: false });
    step(1000);
    // Settled: no frame is pending, and the disc keeps the angle it stopped at,
    // drawn into the SVG (crisp) with the wrapper back to no transform.
    expect(queue).toHaveLength(0);
    expect(wrapperAngle()).toBeUndefined();
    const rest = angleOf();
    expect(rest).not.toBe(0);

    await rerender({ playing: true });
    expect(queue).toHaveLength(1);
    step(5);
    expect(angleOf()).not.toBe(rest);
  });
});
