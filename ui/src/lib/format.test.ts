import { describe, it, expect } from "vitest";
import {
  formatCount,
  formatDuration,
  formatListenTime,
  formatReleaseDate,
  formatTotalDuration,
} from "./format";

describe("formatReleaseDate", () => {
  it("formats full dates and bare years", () => {
    expect(formatReleaseDate("2014-02-11")).toBe("Feb 11, 2014");
    expect(formatReleaseDate("2002-12-01T00:00:00.000+0000")).toBe("Dec 1, 2002");
    expect(formatReleaseDate("1999")).toBe("1999");
  });
  it("returns empty for missing or malformed dates", () => {
    expect(formatReleaseDate(null)).toBe("");
    expect(formatReleaseDate("soon")).toBe("");
    expect(formatReleaseDate("2014-13-01")).toBe("2014");
  });
});

describe("formatListenTime", () => {
  it("reads as an amount of time, never a clock", () => {
    expect(formatListenTime(45_000)).toBe("45 s");
    expect(formatListenTime(42 * 60_000 + 30_000)).toBe("42 min");
    expect(formatListenTime((3 * 60 + 5) * 60_000)).toBe("3 h 05 min");
    expect(formatListenTime((23 * 60 + 43) * 60_000 + 34_000)).toBe("23 h 43 min");
    expect(formatListenTime(52 * 3_600_000)).toBe("2 d 4 h");
    expect(formatListenTime(-5)).toBe("0 s");
  });
});

describe("formatDuration", () => {
  it("formats minutes and seconds with padding", () => {
    expect(formatDuration(276)).toBe("4:36");
    expect(formatDuration(65)).toBe("1:05");
  });
  it("never returns negative", () => {
    expect(formatDuration(-10)).toBe("0:00");
  });
});

describe("formatTotalDuration", () => {
  it("adds an hours segment at one hour and above", () => {
    expect(formatTotalDuration(36968)).toBe("10:16:08");
    expect(formatTotalDuration(3600)).toBe("1:00:00");
  });
  it("keeps the minute form below an hour", () => {
    expect(formatTotalDuration(276)).toBe("4:36");
    expect(formatTotalDuration(65)).toBe("1:05");
  });
  it("never returns negative", () => {
    expect(formatTotalDuration(-10)).toBe("0:00");
  });
});

describe("formatCount", () => {
  it("leaves counts under a thousand exact", () => {
    expect(formatCount(0)).toBe("0");
    expect(formatCount(999)).toBe("999");
  });
  it("compacts thousands with one decimal below a hundred", () => {
    expect(formatCount(1000)).toBe("1K");
    expect(formatCount(9672)).toBe("9.7K");
    expect(formatCount(12000)).toBe("12K");
  });
  it("compacts millions and promotes near-boundary thousands", () => {
    expect(formatCount(999999)).toBe("1M");
    expect(formatCount(1106540)).toBe("1.1M");
  });
  it("promotes near-boundary millions to billions", () => {
    expect(formatCount(999_499_999)).toBe("999M");
    expect(formatCount(999_500_000)).toBe("1B");
  });
  it("compacts billions", () => {
    expect(formatCount(1_000_000_000)).toBe("1B");
  });
});
