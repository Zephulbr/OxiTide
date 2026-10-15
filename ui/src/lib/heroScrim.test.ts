import { describe, expect, it } from "vitest";
import { heroScrim, SCRIM_FLOOR, SCRIM_UNKNOWN } from "./heroScrim";

describe("heroScrim", () => {
  it("keeps deep palettes vivid in the dark theme", () => {
    expect(heroScrim(["#3a2a5c", "#22305a", "#4a2236"], "dark")).toBe(SCRIM_FLOOR);
  });

  it("goes full strength for a pale palette in the dark theme", () => {
    expect(heroScrim(["#c9b8ec", "#9aa8e6", "#ecc3d6"], "dark")).toBe(1);
  });

  it("lets the lightest role decide in the dark theme", () => {
    const deep = heroScrim(["#3a2a5c", "#22305a"], "dark");
    const mixed = heroScrim(["#3a2a5c", "#22305a", "#a99ad0"], "dark");
    expect(mixed).toBeGreaterThan(deep);
    expect(mixed).toBeLessThan(1);
  });

  it("mirrors in the light theme: dark roles need the paper scrim", () => {
    expect(heroScrim(["#f3e6d8", "#e8ecf6", "#f6dde8"], "light")).toBe(SCRIM_FLOOR);
    expect(heroScrim(["#2d2250", "#e8ecf6"], "light")).toBe(1);
  });

  it("falls back when there is no usable palette", () => {
    expect(heroScrim(null, "dark")).toBe(SCRIM_UNKNOWN);
    expect(heroScrim(["rgb(1,2,3)"], "light")).toBe(SCRIM_UNKNOWN);
  });
});
