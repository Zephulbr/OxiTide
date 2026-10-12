import { describe, expect, it } from "vitest";
import { barPercents, dayTick } from "./historyChart";

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
