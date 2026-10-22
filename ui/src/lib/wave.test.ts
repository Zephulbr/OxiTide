import { describe, expect, it } from "vitest";
import { taperAt, waveFillPath, wavePath, waveSamples } from "./wave";

describe("taperAt", () => {
  const taper = { flat: 0.1, ramp: 0.2 };
  it("is 1 without a taper", () => {
    expect(taperAt(0, undefined)).toBe(1);
  });
  it("is flat at both ends, full in the middle, and symmetric", () => {
    expect(taperAt(0, taper)).toBe(0);
    expect(taperAt(0.1, taper)).toBe(0);
    expect(taperAt(0.95, taper)).toBe(0);
    expect(taperAt(0.5, taper)).toBe(1);
    expect(taperAt(0.2, taper)).toBeCloseTo(0.5, 5);
    expect(taperAt(0.2, taper)).toBeCloseTo(taperAt(0.8, taper), 5);
  });
  it("pins a tapered wave's ends to the baseline", () => {
    const samples = waveSamples(42, 192, { flat: 0.05, ramp: 0.05 });
    expect(samples[0]).toBe(0);
    expect(samples[191]).toBe(0);
    expect(samples.slice(0, 9).every((v) => v === 0)).toBe(true);
    expect(waveSamples(42, 192)[0]).toBeGreaterThan(0);
  });
});

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

describe("waveFillPath", () => {
  it("is deterministic for a seed", () => {
    expect(waveFillPath(42)).toBe(waveFillPath(42));
  });

  it("closes the area under the wave on the bottom edge", () => {
    const path = waveFillPath(42);
    expect(path.startsWith("M")).toBe(true);
    expect(path.endsWith("L1,1 L0,1 Z")).toBe(true);
  });

  it("stays in unit coordinates (objectBoundingBox space)", () => {
    const pairs = waveFillPath(7).match(/-?\d+\.?\d*/g)!.map(Number);
    expect(pairs.length).toBe(96 * 2 + 4); // samples + the two closing points
    for (let i = 0; i < pairs.length; i += 2) {
      expect(pairs[i]).toBeGreaterThanOrEqual(0);
      expect(pairs[i]).toBeLessThanOrEqual(1);
      expect(pairs[i + 1]).toBeGreaterThanOrEqual(0);
      expect(pairs[i + 1]).toBeLessThanOrEqual(14 / 12.6);
    }
  });

  it("matches the wavePath shape scaled by 800 x 12.6", () => {
    const samples = waveSamples(42, 96);
    const pairs = waveFillPath(42).match(/-?\d+\.?\d*/g)!.map(Number);
    for (let i = 0; i < samples.length; i++) {
      expect(pairs[i * 2]).toBeCloseTo(i / (samples.length - 1), 3);
      expect(pairs[i * 2 + 1]).toBeCloseTo(13.3 / 12.6 - samples[i], 3);
    }
  });

  it("spans the full width of the box", () => {
    const pairs = waveFillPath(7).match(/-?\d+\.?\d*/g)!.map(Number);
    expect(pairs[0]).toBe(0);
    expect(pairs[(96 - 1) * 2]).toBe(1);
  });
});
