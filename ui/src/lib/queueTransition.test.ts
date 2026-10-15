import { describe, expect, it } from "vitest";
import { GLIDE_MS, queueDock, queueFloat } from "./queueTransition";

function fakeNode(width: number): HTMLElement {
  return { getBoundingClientRect: () => ({ width }) } as unknown as HTMLElement;
}

describe("queue transitions", () => {
  it("dock and float share the glide duration and curve", () => {
    const dock = queueDock(fakeNode(320), { reduced: false });
    const float = queueFloat(fakeNode(320), { reduced: false });
    expect(dock.duration).toBe(GLIDE_MS);
    expect(float.duration).toBe(GLIDE_MS);
    expect(dock.easing).toBe(float.easing);
  });

  it("uses a monotonic ease from 0 to 1 that starts gently", () => {
    const ease = queueDock(fakeNode(320), { reduced: false }).easing!;
    expect(ease(0)).toBe(0);
    expect(ease(1)).toBe(1);
    let last = 0;
    for (let x = 0.05; x < 1; x += 0.05) {
      const y = ease(x);
      expect(y).toBeGreaterThanOrEqual(last);
      last = y;
    }
    // cubic-bezier(0.4, 0, 0.2, 1): slow first 10%, well past halfway at 50%.
    expect(ease(0.1)).toBeLessThan(0.1);
    expect(ease(0.5)).toBeGreaterThan(0.7);
    expect(ease(0.5)).toBeLessThan(0.85);
  });

  it("grows the dock's width with t", () => {
    const css = queueDock(fakeNode(300), { reduced: false }).css!;
    expect(css(0.5, 0.5)).toContain("width: 150px");
  });

  it("falls back to a short fade under reduced motion", () => {
    const dock = queueDock(fakeNode(300), { reduced: true });
    expect(dock.duration).toBe(120);
    expect(dock.css!(0.5, 0.5)).toBe("opacity: 0.5");
  });
});
