import { describe, expect, it } from "vitest";
import { createRubDetector } from "./petting";

/// Feed x positions `stepMs` apart; the times a pet was reported.
function rub(xs: number[], stepMs: number, inside = true, start = 0) {
  const detector = createRubDetector();
  const pets: number[] = [];
  xs.forEach((x, i) => {
    const t = start + i * stepMs;
    if (detector.feed(x, t, inside)) pets.push(t);
  });
  return pets;
}

const backAndForth = (turns: number) => {
  const xs = [0];
  for (let i = 0; i <= turns; i += 1) xs.push(i % 2 === 0 ? 12 : 0);
  return xs;
};

describe("rub detector", () => {
  it("pets once for three quick turns over the cat", () => {
    expect(rub(backAndForth(3), 100)).toHaveLength(1);
  });

  it("needs a new rub after each pet", () => {
    expect(rub(backAndForth(6), 100)).toHaveLength(2);
    expect(rub(backAndForth(5), 100)).toHaveLength(1);
  });

  it("does not pet when the turns are spread over more than the window", () => {
    expect(rub(backAndForth(3), 700)).toHaveLength(0);
  });

  it("does not pet for a straight pass", () => {
    expect(rub([0, 5, 10, 15, 20, 25, 30], 30)).toHaveLength(0);
  });

  it("does not pet outside the cat", () => {
    expect(rub(backAndForth(4), 100, false)).toHaveLength(0);
  });

  it("starts over when the cursor leaves the cat", () => {
    const detector = createRubDetector();
    const feed = (x: number, t: number, inside = true) => detector.feed(x, t, inside);
    [0, 12, 0, 12].forEach((x, i) => feed(x, i * 100));
    feed(200, 400, false);
    expect(feed(0, 500)).toBe(false);
    expect(feed(12, 600)).toBe(false);
  });

  it("ignores jitter smaller than a couple of pixels", () => {
    expect(rub([0, 1, 0, 1, 0, 1, 0, 1], 50)).toHaveLength(0);
  });
});
