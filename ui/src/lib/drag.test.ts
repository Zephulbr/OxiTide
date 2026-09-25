import { describe, expect, it } from "vitest";
import { dropTarget, normalizeDrop } from "./drag";

describe("dropTarget", () => {
  it("maps the top of a row to the slot before it", () => {
    expect(dropTarget(0, 0, 0, 36, 10)).toBe(0);
  });

  it("rounds to the nearer edge", () => {
    expect(dropTarget(17, 0, 0, 36, 10)).toBe(0);
    expect(dropTarget(19, 0, 0, 36, 10)).toBe(1);
  });

  it("accounts for scroll and container position", () => {
    expect(dropTarget(100 + 720, 100, 720, 36, 100)).toBe(40);
  });

  it("accounts for a sticky header offset", () => {
    expect(dropTarget(50, 0, 0, 36, 10, 36)).toBe(0);
    expect(dropTarget(90, 0, 0, 36, 10, 36)).toBe(2);
  });

  it("clamps to the list bounds", () => {
    expect(dropTarget(-50, 0, 0, 36, 3)).toBe(0);
    expect(dropTarget(9999, 0, 0, 36, 3)).toBe(3);
  });

  it("returns zero for an empty list", () => {
    expect(dropTarget(200, 0, 0, 36, 0)).toBe(0);
  });
});

describe("normalizeDrop", () => {
  it("shifts a downward target up by one", () => {
    expect(normalizeDrop(0, 3)).toBe(2);
    expect(normalizeDrop(2, 5)).toBe(4);
  });

  it("leaves upward and self targets unchanged", () => {
    expect(normalizeDrop(5, 2)).toBe(2);
    expect(normalizeDrop(3, 3)).toBe(3);
    expect(normalizeDrop(3, 4)).toBe(3);
  });
});
