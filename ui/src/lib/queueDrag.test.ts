import { describe, expect, it } from "vitest";
import { dragTarget, edgeScroll, rowShift } from "./queueDrag";

describe("dragTarget", () => {
  it("maps the row center to the slot under it, clamped to the list", () => {
    expect(dragTarget(10, 50, 5)).toBe(0);
    expect(dragTarget(125, 50, 5)).toBe(2);
    expect(dragTarget(-40, 50, 5)).toBe(0);
    expect(dragTarget(900, 50, 5)).toBe(4);
    expect(dragTarget(10, 50, 0)).toBe(0);
  });
});

describe("rowShift", () => {
  it("slides rows up when dragging down", () => {
    expect([0, 1, 2, 3, 4].map((i) => rowShift(i, 1, 3))).toEqual([0, 0, -1, -1, 0]);
  });

  it("slides rows down when dragging up", () => {
    expect([0, 1, 2, 3, 4].map((i) => rowShift(i, 3, 1))).toEqual([0, 1, 1, 0, 0]);
  });

  it("moves nothing while the row is over its own slot", () => {
    expect([0, 1, 2].map((i) => rowShift(i, 1, 1))).toEqual([0, 0, 0]);
  });
});

describe("edgeScroll", () => {
  it("scrolls toward an edge, faster deeper in", () => {
    expect(edgeScroll(300, 0, 600)).toBe(0);
    expect(edgeScroll(40, 0, 600)).toBeLessThan(0);
    expect(edgeScroll(0, 0, 600)).toBe(-18);
    expect(edgeScroll(580, 0, 600)).toBeGreaterThan(0);
    expect(edgeScroll(600, 0, 600)).toBe(18);
  });

  it("stays off in a list too short to have a middle", () => {
    expect(edgeScroll(10, 0, 100)).toBe(0);
  });
});
