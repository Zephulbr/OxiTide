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

/// jsdom has no Web Animations: a stand-in that records what was asked.
type FakeAnimation = {
  keyframes: Keyframe[];
  options: KeyframeAnimationOptions;
  currentTime: number;
  playbackRate: number;
  cancelled: boolean;
  cancel(): void;
};
let animations: FakeAnimation[] = [];

beforeEach(() => {
  queue = [];
  now = 0;
  animations = [];
  vi.spyOn(performance, "now").mockImplementation(() => now);
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    queue.push(callback);
    return queue.length;
  });
  vi.stubGlobal("cancelAnimationFrame", () => {
    queue = [];
  });
  HTMLElement.prototype.animate = function (keyframes, options) {
    const a: FakeAnimation = {
      keyframes: keyframes as Keyframe[],
      options: options as KeyframeAnimationOptions,
      currentTime: 0,
      playbackRate: 1,
      cancelled: false,
      cancel() {
        this.cancelled = true;
      },
    };
    animations.push(a);
    return a as unknown as Animation;
  };
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  delete (HTMLElement.prototype as Partial<HTMLElement>).animate;
});

const svgAngle = () => Number(/rotate\(([-\d.]+)/.exec(document.querySelector("g")?.getAttribute("transform") ?? "")?.[1] ?? 0);
const live = () => animations.filter((a) => !a.cancelled);

describe("NowPlayingMark", () => {
  it("runs no frame loop and no animation while paused at rest", () => {
    render(NowPlayingMark, { playing: false });
    expect(queue).toHaveLength(0);
    expect(animations).toHaveLength(0);
  });

  it("turns in 30 steps a second on the compositor; JS runs only while the speed eases", () => {
    render(NowPlayingMark, { playing: true });
    expect(live()).toHaveLength(1);
    const [a] = live();
    expect(a.options).toMatchObject({ duration: 2000, iterations: Infinity, easing: "steps(60, end)" });
    expect(a.keyframes).toEqual([{ transform: "rotate(0deg)" }, { transform: "rotate(360deg)" }]);
    // Easing in: the rate climbs from rest.
    expect(a.playbackRate).toBe(0);
    step(5);
    expect(a.playbackRate).toBeGreaterThan(0);
    expect(a.playbackRate).toBeLessThan(1);
    // Up to speed, the loop ends: the compositor turns it alone.
    step(200);
    expect(a.playbackRate).toBe(1);
    expect(queue).toHaveLength(0);
    // The SVG stays upright while the wrapper turns.
    expect(svgAngle()).toBe(0);
  });

  it("eases to a stop on pause, keeps the stepped angle in the SVG, and resumes from it", async () => {
    const { rerender } = render(NowPlayingMark, { playing: true });
    step(200);
    const [a] = live();
    a.currentTime = 510; // a quarter turn and a bit: step 15 of 60
    await rerender({ playing: false });
    step(5);
    expect(a.playbackRate).toBeGreaterThan(0);
    expect(a.playbackRate).toBeLessThan(1);
    step(1000);
    // Settled: no frame pending, no animation, the angle drawn into the SVG.
    expect(queue).toHaveLength(0);
    expect(a.cancelled).toBe(true);
    expect(svgAngle()).toBe(90);

    await rerender({ playing: true });
    expect(live()).toHaveLength(1);
    expect(live()[0].keyframes[0]).toEqual({ transform: "rotate(90deg)" });
    expect(svgAngle()).toBe(0);
  });

  it("keeps the same animation when play returns mid-ease", async () => {
    const { rerender } = render(NowPlayingMark, { playing: true });
    step(200);
    await rerender({ playing: false });
    step(5);
    const slowed = live()[0].playbackRate;
    expect(slowed).toBeLessThan(1);
    await rerender({ playing: true });
    step(200);
    expect(animations).toHaveLength(1);
    expect(live()[0].playbackRate).toBe(1);
    expect(queue).toHaveLength(0);
  });
});
