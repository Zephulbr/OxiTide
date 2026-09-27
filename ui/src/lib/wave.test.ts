import { describe, expect, it } from "vitest";
import { wavePath, waveSamples } from "./wave";

describe("waveSamples", () => {
  it("is deterministic for a seed", () => {
    expect(waveSamples(42, 32)).toEqual(waveSamples(42, 32));
  });
  it("varies with the seed", () => {
    expect(waveSamples(1, 32)).not.toEqual(waveSamples(2, 32));
  });
  it("stays within 0..1", () => {
    for (const v of waveSamples(7, 96)) {
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThanOrEqual(1);
    }
  });
});

describe("wavePath", () => {
  it("builds an svg path inside the viewbox", () => {
    const path = wavePath(42, 800, 14);
    expect(path.startsWith("M")).toBe(true);
    const coords = path.match(/-?\d+\.?\d*/g)!.map(Number);
    const ys = coords.filter((_, i) => i % 2 === 1);
    for (const y of ys) {
      expect(y).toBeGreaterThanOrEqual(0);
      expect(y).toBeLessThanOrEqual(14);
    }
  });
});
