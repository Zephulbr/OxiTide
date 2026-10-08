import { describe, expect, it } from "vitest";
import type { QueueItem } from "../api";
import {
  bandFraction,
  dbfs,
  fallStep,
  ledCount,
  needleAngle,
  peakFraction,
  peakState,
  peakStep,
  poolBands,
  vuPosition,
  vuStep,
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
    expect(sourceOf({ name: "playlist", id: "p" })).toBe("playlists");
    expect(sourceOf({ name: "folder", id: "f" })).toBe("playlists");
    expect(sourceOf({ name: "album", id: 1 })).toBe("library");
    expect(sourceOf({ name: "history", tab: "stats" })).toBe("library");
    expect(sourceOf({ name: "settings" })).toBe("settings");
  });
  it("shows the library tray only on collection routes", () => {
    expect(libraryKeyOf({ name: "profiles" })).toBe("Artists");
    expect(libraryKeyOf({ name: "album", id: 1 })).toBeNull();
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
  it("VU integrates to ~99% in 300 ms", () => {
    let v = 0;
    for (let t = 0; t < 0.3; t += 1 / 60) v = vuStep(v, 1, 1 / 60);
    expect(v).toBeGreaterThan(0.98);
  });
  it("0 VU sits at −18 dBFS RMS and the needle spans ±46°", () => {
    // The 0 VU mark is drawn at (1 − .1)/(10^(3/20) − .1) ≈ .686 of the arc.
    const zero = vuPosition(10 ** (-18 / 20));
    expect(zero).toBeCloseTo(0.9 / (10 ** (3 / 20) - 0.1), 6);
    expect(vuPosition(0)).toBe(-0.04);
    expect(vuPosition(1)).toBe(1.04);
    expect(needleAngle(0)).toBe(-46);
    expect(needleAngle(1)).toBe(46);
  });
  it("peaks attack instantly, hold 1.5 s, then fall", () => {
    let s = peakStep(peakState(), -6, 0.016, 0);
    expect(s.db).toBe(-6);
    s = peakStep(s, null, 1, 1000);
    expect(s.db).toBeCloseTo(-30);
    expect(s.holdDb).toBe(-6);
    s = peakStep(s, null, 1, 2600);
    expect(s.holdDb).toBeLessThan(-6);
  });
  it("scales, falls, LEDs and pooling", () => {
    expect(dbfs(0)).toBe(-96);
    expect(peakFraction(3)).toBe(1);
    expect(peakFraction(-40)).toBe(0);
    expect(bandFraction(-30)).toBeCloseTo(0.5);
    expect(fallStep(0.2, 0.8, 1.4, 0.1)).toBe(0.8);
    expect(fallStep(0.8, 0, 1.4, 0.1)).toBeCloseTo(0.66);
    expect(ledCount(-100)).toBe(0);
    expect(ledCount(0)).toBe(17);
    expect(ledCount(5)).toBe(20);
    expect(poolBands([1, 5, 2, 8], 2)).toEqual([5, 8]);
  });
});
