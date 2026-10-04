import { describe, expect, it } from "vitest";
import { barPercents } from "./historyChart";

describe("barPercents", () => {
  it("scales against the tallest day", () => {
    expect(barPercents([0, 2, 4])).toEqual([0, 50, 100]);
  });

  it("uses a floor of one so all-zero days stay flat", () => {
    expect(barPercents([0, 0, 0])).toEqual([0, 0, 0]);
  });
});
