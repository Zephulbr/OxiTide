import { describe, expect, it } from "vitest";
import type { QueueItem } from "../api";
import {
  bandFraction,
  dbfs,
  fallStep,
  ledCount,
  needleAngle,
  needleState,
  needleStep,
  peakFraction,
  peakState,
  peakStep,
  poolBands,
  programmeDb,
  REFERENCE_DBFS,
  accumulate,
  recorderAccumulator,
  recorderFraction,
  recorderPoint,
  vuPosition,
} from "./ballistics";
import { calendar, clock, formatBadge, formatLine, remainingMs, repeatAnnunciator, upNext } from "./deck";
import { libraryKeyOf, sourceOf } from "./sources";

const item = (id: number, secs = 100): QueueItem => ({
  track_id: id,
  title: `T${id}`,
  artist: "A",
  album: "B",
  duration_secs: secs,
});

describe("sourceOf", () => {
  it("maps routes to the lit source key", () => {
    expect(sourceOf({ name: "home" })).toBe("home");
    expect(sourceOf({ name: "section", source: "explore", title: "x" })).toBe("explore");
    expect(sourceOf({ name: "playlist", id: "p" })).toBe("library");
    expect(sourceOf({ name: "folder", id: "f" })).toBe("library");
    expect(sourceOf({ name: "album", id: 1 })).toBe("library");
    expect(sourceOf({ name: "feed" })).toBe("feed");
    expect(sourceOf({ name: "history", tab: "stats" })).toBe("history");
    expect(sourceOf({ name: "settings" })).toBe("settings");
  });
  it("shows the library tray only on collection routes", () => {
    expect(libraryKeyOf({ name: "profiles" })).toBe("Profiles");
    expect(libraryKeyOf({ name: "folder", id: "f" })).toBe("Playlists");
    expect(libraryKeyOf({ name: "album", id: 1 })).toBeNull();
    expect(libraryKeyOf({ name: "playlist", id: "p" })).toBeNull();
  });
});

describe("deck helpers", () => {
  const state = {
    queue: [item(1), item(2), item(3), item(4), item(5)],
    current: item(2),
    current_index: null,
    position_ms: 40_000,
    duration_ms: 100_000,
  };
  it("up next follows the current track without wrapping", () => {
    expect(upNext(state).map((i) => i.track_id)).toEqual([3, 4, 5]);
    expect(upNext({ ...state, current: item(5) }).length).toBe(0);
  });
  it("remaining counts the rest of this track plus the queue after it", () => {
    expect(remainingMs(state)).toBe(60_000 + 300_000);
  });
  it("calendar keeps the current track in its window of 20", () => {
    const cells = calendar(5, 1);
    expect(cells[1].state).toBe("current");
    expect(cells[4].state).toBe("present");
    expect(cells[5].state).toBe("absent");
    const later = calendar(30, 24);
    expect(later[0].n).toBe(21);
    expect(later[4].state).toBe("current");
    expect(later[9].state).toBe("present");
    expect(later[10].state).toBe("absent");
  });
  it("formats clocks, repeat and format lines", () => {
    expect(clock(62_000)).toBe("1:02");
    expect(clock(3_725_000)).toBe("1:02:05");
    expect(repeatAnnunciator("off")).toBeNull();
    expect(repeatAnnunciator("one")).toBe("RPT 1");
    const stream = { codec: "flac", bit_depth: 24, sample_rate: 44_100, channels: 2 };
    expect(formatLine(stream, "exclusive")).toEqual(["FLAC", "24BIT", "44.1kHz", "EXCL"]);
    expect(formatLine(null, "shared")).toEqual(["SHRD"]);
    expect(formatBadge({ ...stream, sample_rate: 96_000 })).toBe("24/96");
    expect(formatBadge(null)).toBeNull();
  });
});

describe("ballistics", () => {
  const settle = (target: number, seconds: number) => {
    let n = needleState();
    let peak = n.x;
    for (let t = 0; t < seconds; t += 1 / 60) {
      n = needleStep(n, target, 1 / 60);
      peak = Math.max(peak, n.x);
    }
    return { n, peak };
  };
  it("the VU needle settles in ~300 ms with a small overshoot, like a real movement", () => {
    const at = settle(0.7, 0.3).n.x;
    expect(at).toBeGreaterThan(0.7 * 0.9);
    const { n, peak } = settle(0.7, 2);
    expect(n.x).toBeCloseTo(0.7, 3);
    expect(peak).toBeGreaterThan(0.7);
    expect(peak).toBeLessThan(0.7 * 1.03);
  });
  it("the needle never passes its pins", () => {
    expect(settle(5, 1).peak).toBeLessThanOrEqual(1.04);
    expect(needleStep({ x: -0.04, v: -3 }, -5, 0.1).x).toBe(-0.04);
  });
  it("0 VU and 0 dB sit at the programme reference, not full scale", () => {
    // The 0 VU mark is drawn at (1 − .1)/(10^(3/20) − .1) ≈ .686 of the arc.
    const zero = vuPosition(10 ** (REFERENCE_DBFS / 20));
    expect(zero).toBeCloseTo(0.9 / (10 ** (3 / 20) - 0.1), 6);
    expect(programmeDb(10 ** (REFERENCE_DBFS / 20))).toBeCloseTo(0, 6);
    // A typical master (short-term RMS −14 dBFS) reads −4 VU: mid-scale, not pinned.
    expect(vuPosition(10 ** (-14 / 20))).toBeGreaterThan(0.3);
    expect(vuPosition(10 ** (-14 / 20))).toBeLessThan(0.6);
    expect(vuPosition(0)).toBe(-0.04);
    expect(vuPosition(1)).toBe(1.04);
    expect(needleAngle(0)).toBe(-46);
    expect(needleAngle(1)).toBe(46);
  });
  it("programme meters rise fast, fall at 13 dB/s, and hold 1.5 s", () => {
    let s = peakState();
    for (let t = 0; t <= 100; t += 16) s = peakStep(s, -6, 0.016, t);
    expect(s.db).toBeGreaterThan(-6.5);
    s = peakStep(s, null, 1, 1100);
    expect(s.db).toBeCloseTo(-19, 0);
    expect(s.holdDb).toBeGreaterThan(-6.5);
    s = peakStep(s, null, 1, 2700);
    expect(s.holdDb).toBeLessThan(-6.5);
  });
  it("level recorder: mean-power points per interval, none for silence", () => {
    const acc = recorderAccumulator();
    expect(recorderPoint(acc)).toBeNull();
    accumulate(acc, [0, 0]);
    expect(recorderPoint(acc)).toBeNull(); // paused: the paper stands still
    const ref = 10 ** (REFERENCE_DBFS / 20);
    const loud = recorderAccumulator();
    accumulate(loud, [ref, ref / 10]);
    accumulate(loud, [ref, ref / 10]);
    const [l, r] = recorderPoint(loud)!;
    expect(l).toBeCloseTo(0, 6);
    expect(r).toBeCloseTo(-20, 6);
    expect(recorderFraction(-30)).toBe(0);
    expect(recorderFraction(6)).toBe(1);
    expect(recorderFraction(-12)).toBeCloseTo(0.5);
  });
  it("scales, falls, LEDs and pooling", () => {
    expect(dbfs(0)).toBe(-96);
    expect(peakFraction(3)).toBe(1);
    expect(peakFraction(-40)).toBe(0);
    expect(bandFraction(-30)).toBeCloseTo(0.5);
    expect(fallStep(0.2, 0.8, 1.4, 0.1)).toBe(0.8);
    expect(fallStep(0.2, 0.8, 1.4, 0.05, 0.05)).toBeLessThan(0.8);
    expect(fallStep(0.8, 0, 1.4, 0.1)).toBeCloseTo(0.66);
    expect(ledCount(-100)).toBe(0);
    expect(ledCount(0)).toBe(17);
    expect(ledCount(5)).toBe(20);
    expect(poolBands([1, 5, 2, 8], 2)).toEqual([5, 8]);
  });
});
