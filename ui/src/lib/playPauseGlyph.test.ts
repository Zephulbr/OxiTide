import { describe, expect, it } from "vitest";
import source from "./components/PlayPauseGlyph.svelte?raw";

const morphPaths = [...source.matchAll(/d:\s*path\("([^"]*)"\)/g)].map((match) => match[1]);

type Point = [number, number];

function commands(d: string): string[] {
  return d.match(/[A-Za-z]/g) ?? [];
}

function parseQuads(d: string): Point[][] {
  return d
    .split("M")
    .slice(1)
    .map((subpath) => {
      const numbers = (subpath.match(/-?\d+(?:\.\d+)?/g) ?? []).map(Number);
      const points: Point[] = [];
      for (let i = 0; i < numbers.length; i += 2) {
        points.push([numbers[i], numbers[i + 1]]);
      }
      return points;
    });
}

function distance(a: Point, b: Point): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1]);
}

const quadsByPath = morphPaths.map(parseQuads);
const playIndex = quadsByPath.findIndex((quads) => distance(quads[0][1], quads[0][2]) < 1);
const playPath = morphPaths[playIndex];
const pausePath = morphPaths[1 - playIndex];

describe("play/pause morph glyph", () => {
  it("defines exactly the play and pause path states in CSS", () => {
    expect(morphPaths).toHaveLength(2);
  });

  it("keeps matching command structure so the d transition can interpolate", () => {
    expect(commands(playPath)).toEqual(commands(pausePath));
  });

  it("uses two closed quads in both states", () => {
    for (const d of morphPaths) {
      expect(commands(d).join("")).toBe("MLLLZMLLLZ");
    }
  });

  it("draws the play state as an equilateral triangle", () => {
    const [a, tipA, tipB, b] = quadsByPath[playIndex][0];
    const tip: Point = [(tipA[0] + tipB[0]) / 2, (tipA[1] + tipB[1]) / 2];
    const sides = [distance(a, tip), distance(tip, b), distance(a, b)];
    for (const side of sides) {
      expect(side).toBeGreaterThan(10);
      expect(Math.abs(side - sides[0])).toBeLessThan(0.01);
    }
  });

  it("draws the pause state as two tall bars", () => {
    for (const [p0, p1, p2] of quadsByPath[1 - playIndex]) {
      expect(distance(p1, p2)).toBeGreaterThan(distance(p0, p1));
    }
  });

  it("shows pause bars while playing and the triangle while paused", () => {
    const baseRule = source.match(/\.glyph\s*\{[^}]*d:\s*path\("([^"]*)"\)/)?.[1];
    const playingRule = source.match(/\.glyph\.playing\s*\{[^}]*d:\s*path\("([^"]*)"\)/)?.[1];
    const attributePlayingBranch = source.match(/d=\{playing\s*\?\s*"([^"]*)"/)?.[1];
    expect(baseRule).toBe(playPath);
    expect(playingRule).toBe(pausePath);
    expect(attributePlayingBranch).toBe(pausePath);
    expect(source).toContain("class:playing={playing}");
  });
});
