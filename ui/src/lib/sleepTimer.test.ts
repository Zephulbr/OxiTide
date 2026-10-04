import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createSleepTimer, SLEEP_FADE_MS } from "./sleepTimer.svelte";

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-10-04T12:00:00Z"));
});

afterEach(() => {
  vi.useRealTimers();
});

function setup() {
  const fires: number[] = [];
  const timer = createSleepTimer((fadeMs) => fires.push(fadeMs));
  return { timer, fires };
}

describe("sleep timer", () => {
  it("counts a duration timer down and fires the fade once at zero", () => {
    const { timer, fires } = setup();
    timer.armMinutes(15);
    expect(timer.mode).toBe("duration");
    expect(timer.totalMinutes).toBe(15);
    expect(timer.remainingMs).toBe(15 * 60_000);

    vi.advanceTimersByTime(60_000);
    expect(timer.remainingMs).toBe(14 * 60_000);
    expect(fires).toEqual([]);

    vi.advanceTimersByTime(14 * 60_000 + 1_000);
    expect(fires).toEqual([SLEEP_FADE_MS]);
    expect(timer.mode).toBe("off");
    expect(timer.remainingMs).toBeNull();
  });

  it("cancel disarms a running timer without firing", () => {
    const { timer, fires } = setup();
    timer.armMinutes(1);
    vi.advanceTimersByTime(30_000);
    timer.cancel();
    vi.advanceTimersByTime(120_000);
    expect(fires).toEqual([]);
    expect(timer.mode).toBe("off");
  });

  it("end-of-track fires the fade when five seconds remain and disarms", () => {
    const { timer, fires } = setup();
    timer.armEndOfTrack();
    timer.observe(54_600, 60_000, true, 1);
    expect(fires).toEqual([]);
    timer.observe(55_200, 60_000, true, 1);
    expect(fires).toEqual([SLEEP_FADE_MS]);
    expect(timer.mode).toBe("off");
    timer.observe(55_800, 60_000, true, 1);
    expect(fires).toEqual([SLEEP_FADE_MS]);
  });

  it("end-of-track ignores pauses and zero durations", () => {
    const { timer, fires } = setup();
    timer.armEndOfTrack();
    timer.observe(59_000, 60_000, false, 1);
    timer.observe(0, 0, true, 1);
    expect(fires).toEqual([]);
  });

  it("an empty queue disarms the timer", () => {
    const { timer, fires } = setup();
    timer.armMinutes(30);
    timer.observe(0, 0, false, 0);
    expect(timer.mode).toBe("off");
    vi.advanceTimersByTime(31 * 60_000);
    expect(fires).toEqual([]);
  });

  it("a natural queue end disarms the timer", () => {
    const { timer, fires } = setup();
    timer.armMinutes(30);
    timer.observe(60_000, 60_000, false, 3);
    expect(timer.mode).toBe("off");
    vi.advanceTimersByTime(31 * 60_000);
    expect(fires).toEqual([]);
  });

  it("ignores non-positive custom durations", () => {
    const { timer } = setup();
    timer.armMinutes(0);
    expect(timer.mode).toBe("off");
    timer.armMinutes(-5);
    expect(timer.mode).toBe("off");
  });

  it("re-arming replaces the previous duration", () => {
    const { timer, fires } = setup();
    timer.armMinutes(60);
    vi.advanceTimersByTime(10_000);
    timer.armMinutes(15);
    expect(timer.totalMinutes).toBe(15);
    // The old 60-minute deadline must not fire: 15 minutes from the re-arm.
    vi.advanceTimersByTime(14 * 60_000);
    expect(fires).toEqual([]);
    vi.advanceTimersByTime(1 * 60_000 + 1_000);
    expect(fires).toEqual([SLEEP_FADE_MS]);
  });

  it("end-of-track reports no countdown", () => {
    const { timer } = setup();
    timer.armEndOfTrack();
    expect(timer.remainingMs).toBeNull();
    expect(timer.totalMinutes).toBeNull();
  });
});
