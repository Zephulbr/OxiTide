import { describe, expect, it } from "vitest";
import type { CoverPalette } from "../api";
import { contrastRatio, coverLamp, hexToOklch, LAMP_PRESETS, lampFor, oklchToSrgb, vfdLightness } from "./lamp";

const palette = (primary: string, secondary = "#202020", tertiary = "#303030", backdrop = "#101010"): CoverPalette => ({
  primary,
  on_primary: "#000000",
  secondary,
  tertiary,
  backdrop,
  ceiling_l: 0.1,
});

describe("hexToOklch", () => {
  it("round-trips through sRGB", () => {
    const [L, C, h] = hexToOklch("#3f7fb5")!;
    const [r, g, b] = oklchToSrgb(L, C, h);
    expect(Math.round(r * 255)).toBe(0x3f);
    expect(Math.round(g * 255)).toBe(0x7f);
    expect(Math.round(b * 255)).toBe(0xb5);
  });
  it("rejects non-hex input", () => {
    expect(hexToOklch("rgb(1,2,3)")).toBeNull();
    expect(hexToOklch("#abc")).toBeNull();
  });
});

describe("coverLamp", () => {
  it("takes the hue of the most chromatic role at a fixed chroma", () => {
    const lamp = coverLamp(palette("#3f7fb5"));
    expect(lamp.c).toBe(0.15);
    expect(lamp.h).toBeGreaterThan(230);
    expect(lamp.h).toBeLessThan(265);
  });
  it("falls back to amber for grey covers and missing palettes", () => {
    expect(coverLamp(palette("#c9c3b6", "#6c6a63", "#808080", "#202020"))).toEqual(LAMP_PRESETS.amber);
    expect(coverLamp(null)).toEqual(LAMP_PRESETS.amber);
  });
  it("lampFor uses presets unless the setting is cover", () => {
    expect(lampFor("blue", palette("#ff0000"))).toEqual(LAMP_PRESETS.blue);
    expect(lampFor("cover", null)).toEqual(LAMP_PRESETS.amber);
  });
});

/// rack.css: --rack-vfd = oklch(.8·dim c h) on --rack-vfd-bg = oklch(.115 c·.1 h).
/// VFD text must clear 4.5:1 for every preset down to the 40% brightness floor.
describe("lamp contrast floors", () => {
  for (const [name, { h, c }] of Object.entries(LAMP_PRESETS)) {
    for (const dim of [0.4, 1]) {
      it(`${name} VFD text at ${dim * 100}% brightness`, () => {
        const text = oklchToSrgb(vfdLightness(dim), c, h);
        const bg = oklchToSrgb(0.115, c * 0.1, h);
        expect(contrastRatio(text, bg)).toBeGreaterThanOrEqual(4.5);
      });
    }
  }
  it("screen accent on a pane clears 4.5:1 for every cover hue", () => {
    const pane = [0x0a / 255, 0x09 / 255, 0x08 / 255] as [number, number, number];
    for (let h = 0; h < 360; h += 15) {
      expect(contrastRatio(oklchToSrgb(0.79, 0.15, h), pane)).toBeGreaterThanOrEqual(4.5);
    }
  });
});
