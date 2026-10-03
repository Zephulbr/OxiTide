import { describe, expect, it } from "vitest";
import { tiltFromPointer } from "./tilt";

/// A 200x200 card at the origin; the pointer coordinates below are local.
const rect = { left: 0, top: 0, width: 200, height: 200 };
const MAX_TILT = 12;

describe("tiltFromPointer", () => {
  it("is flat with no glare at the center", () => {
    const t = tiltFromPointer(rect, 100, 100, MAX_TILT);
    expect(t.rx).toBe(0);
    expect(t.ry).toBe(0);
    expect(t.gx).toBe(50);
    expect(t.gy).toBe(50);
    expect(t.glare).toBe(0);
  });

  it("tilts right and mirrors the glare left at the right edge", () => {
    const t = tiltFromPointer(rect, 200, 100, MAX_TILT);
    expect(t.ry).toBe(MAX_TILT);
    expect(t.rx).toBe(0);
    expect(t.gx).toBe(0);
    expect(t.gy).toBe(50);
    expect(t.glare).toBe(1);
  });

  it("caps the combined tilt at maxTilt in the top-left corner", () => {
    const t = tiltFromPointer(rect, 0, 0, MAX_TILT);
    expect(t.rx).toBeCloseTo(MAX_TILT / Math.SQRT2, 6);
    expect(t.ry).toBeCloseTo(-MAX_TILT / Math.SQRT2, 6);
    expect(Math.hypot(t.rx, t.ry)).toBeCloseTo(MAX_TILT, 6);
    expect(t.gx).toBe(100);
    expect(t.gy).toBe(100);
    expect(t.glare).toBe(1);
  });

  it("tilts down and right at the bottom-right corner", () => {
    const t = tiltFromPointer(rect, 200, 200, MAX_TILT);
    expect(t.rx).toBeCloseTo(-MAX_TILT / Math.SQRT2, 6);
    expect(t.ry).toBeCloseTo(MAX_TILT / Math.SQRT2, 6);
    expect(Math.hypot(t.rx, t.ry)).toBeCloseTo(MAX_TILT, 6);
    expect(t.gx).toBe(0);
    expect(t.gy).toBe(0);
    expect(t.glare).toBe(1);
  });

  it("tilts down at the bottom edge with the glare at the top", () => {
    const t = tiltFromPointer(rect, 100, 200, MAX_TILT);
    expect(t.rx).toBe(-MAX_TILT);
    expect(t.ry).toBe(0);
    expect(t.gx).toBe(50);
    expect(t.gy).toBe(0);
    expect(t.glare).toBe(1);
  });

  it("scales the tilt by maxTilt", () => {
    const t = tiltFromPointer(rect, 150, 100, 10);
    expect(t.ry).toBe(5);
  });

  it("tilts proportionally to the distance from center", () => {
    const t = tiltFromPointer(rect, 125, 100, MAX_TILT); // a quarter right of center
    expect(t.ry).toBeCloseTo(0.25 * MAX_TILT, 6);
    expect(t.rx).toBe(0);
  });

  it("clamps pointers outside the card to its edges", () => {
    const t = tiltFromPointer(rect, 500, 100, MAX_TILT);
    expect(t.ry).toBe(MAX_TILT);
    expect(t.gx).toBe(0);
  });

  it("stays flat for a zero-size box", () => {
    const t = tiltFromPointer({ left: 0, top: 0, width: 0, height: 0 }, 10, 10, MAX_TILT);
    expect(t).toEqual({ rx: 0, ry: 0, gx: 50, gy: 50, glare: 0 });
  });
});
