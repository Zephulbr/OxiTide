import { describe, expect, it } from "vitest";
import {
  BOB_FRAMES,
  CATJAM_SIZE,
  COAT_IDS,
  COATS,
  DEFAULT_COAT,
  FRAMES,
  IDLE_INDEX,
  SWAY_FRAMES,
  SWAY_OFFSET,
  SWAY_PERIOD_MS,
  bobFrame,
  clampCatjamScale,
  isCoat,
  swayFrame,
} from "./catjamSprite";

function luminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  const channel = (v: number) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel((n >> 16) & 255) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255);
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

describe("catjam sprite", () => {
  it("has 8 bob frames, 2 sway frames and an idle frame", () => {
    expect(BOB_FRAMES).toHaveLength(8);
    expect(SWAY_FRAMES).toHaveLength(2);
    expect(FRAMES).toHaveLength(11);
    expect(IDLE_INDEX).toBe(10);
  });

  it("draws every frame on a 32x32 grid", () => {
    for (const frame of FRAMES) {
      expect(frame).toHaveLength(CATJAM_SIZE);
      for (const row of frame) expect(row).toHaveLength(CATJAM_SIZE);
    }
  });

  it("gives every frame a distinct picture", () => {
    const pictures = new Set(FRAMES.map((f) => f.join("\n")));
    expect(pictures.size).toBe(FRAMES.length);
  });

  it("walks the bob across two beats and lands on frames 0 and 4", () => {
    expect(bobFrame(0, 0)).toBe(0);
    expect(bobFrame(1, 0)).toBe(4);
    expect(bobFrame(0, 0.99)).toBe(3);
    expect(bobFrame(1, 0.99)).toBe(7);
    expect(bobFrame(2, 0.5)).toBe(2);
    const seen = new Set<number>();
    for (let b = 0; b < 2; b++) for (let p = 0; p < 1; p += 0.125) seen.add(bobFrame(b, p));
    expect([...seen].sort((a, b) => a - b)).toEqual([0, 1, 2, 3, 4, 5, 6, 7]);
  });

  it("handles beats before the anchor", () => {
    expect(bobFrame(-1, 0)).toBe(4);
    expect(bobFrame(-2, 0.3)).toBe(1);
  });

  it("clamps the phase", () => {
    expect(bobFrame(0, 1)).toBe(3);
    expect(bobFrame(0, -0.1)).toBe(0);
  });

  it("alternates the sway frames every half period", () => {
    expect(swayFrame(0)).toBe(SWAY_OFFSET);
    expect(swayFrame(SWAY_PERIOD_MS / 2)).toBe(SWAY_OFFSET + 1);
    expect(swayFrame(SWAY_PERIOD_MS)).toBe(SWAY_OFFSET);
    expect(swayFrame(-1)).toBe(SWAY_OFFSET + 1);
  });
});

describe("catjam coats", () => {
  const used = new Set(FRAMES.flatMap((f) => f.flatMap((row) => [...row])));

  it("defines a colour for every character the frames use, in every coat", () => {
    for (const id of COAT_IDS) {
      for (const ch of used) expect(ch in COATS[id], `${id} lacks "${ch}"`).toBe(true);
    }
  });

  it("keeps the white coat's markings invisible", () => {
    const white = COATS.white;
    expect(white.s).toBe(white.w);
    expect(white.c).toBe(white.w);
    expect(white.e).toBe(white.k);
    expect(white.l).toBe(white.k);
    expect(white.m).toBe(white.k);
  });

  it("keeps eyes and closed lids readable against the fur", () => {
    for (const id of COAT_IDS) {
      const coat = COATS[id];
      expect(contrast(coat.e!, coat.w!), `${id} eyes`).toBeGreaterThanOrEqual(3);
      expect(contrast(coat.l!, coat.w!), `${id} lids`).toBeGreaterThanOrEqual(2.5);
    }
  });

  it("lists every coat it has a palette and a label for", () => {
    expect([...COAT_IDS].sort()).toEqual(Object.keys(COATS).sort());
  });

  it("caps the size between 1× and 2×", () => {
    expect(clampCatjamScale(50)).toBe(100);
    expect(clampCatjamScale(150)).toBe(150);
    expect(clampCatjamScale(999)).toBe(200);
    expect(clampCatjamScale(Number.NaN)).toBe(125);
  });

  it("recognises coat ids and defaults to white", () => {
    expect(DEFAULT_COAT).toBe("white");
    expect(isCoat("tuxedo")).toBe(true);
    expect(isCoat("calico")).toBe(false);
    expect(isCoat(3)).toBe(false);
  });
});
