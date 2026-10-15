import { describe, expect, it } from "vitest";
import { barPercents, dayPart, dayTick, hourLabel, peakIndex, percentOf } from "./historyChart";

describe("peakIndex", () => {
  it("finds the first largest value", () => {
    expect(peakIndex([1, 3, 2, 3])).toBe(1);
  });

  it("is -1 when nothing was played", () => {
    expect(peakIndex([0, 0])).toBe(-1);
    expect(peakIndex([])).toBe(-1);
  });
});

describe("hour helpers", () => {
  it("labels and phrases hours", () => {
    expect(hourLabel(7)).toBe("07:00");
    expect(dayPart(2)).toBe("late at night");
    expect(dayPart(9)).toBe("in the morning");
    expect(dayPart(14)).toBe("in the afternoon");
    expect(dayPart(19)).toBe("in the evening");
    expect(dayPart(23)).toBe("at night");
  });

  it("rounds shares and guards an empty whole", () => {
    expect(percentOf(1, 3)).toBe(33);
    expect(percentOf(5, 0)).toBe(0);
  });
});

describe("dayTick", () => {
  it("labels a local calendar day without timezone shifts", () => {
    expect(dayTick("2026-09-11")).toBe("Sep 11");
    expect(dayTick("2026-01-01")).toBe("Jan 1");
    expect(dayTick("garbage")).toBe("garbage");
  });
});

describe("barPercents", () => {
  it("scales against the tallest day", () => {
    expect(barPercents([0, 2, 4])).toEqual([0, 50, 100]);
  });

  it("uses a floor of one so all-zero days stay flat", () => {
    expect(barPercents([0, 0, 0])).toEqual([0, 0, 0]);
  });
});
