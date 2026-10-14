import { describe, expect, it } from "vitest";
import { seekByKey, seekRatio } from "./seek";

describe("seekRatio", () => {
  it("maps a pointer position to a 0..1 ratio", () => {
    expect(seekRatio(150, 100, 100)).toBe(0.5);
    expect(seekRatio(100, 100, 100)).toBe(0);
    expect(seekRatio(200, 100, 100)).toBe(1);
  });

  it("clamps outside the track", () => {
    expect(seekRatio(50, 100, 100)).toBe(0);
    expect(seekRatio(250, 100, 100)).toBe(1);
  });

  it("returns 0 for a zero-width track", () => {
    expect(seekRatio(150, 100, 0)).toBe(0);
  });
});

describe("seekByKey", () => {
  it("steps ±5 s", () => {
    expect(seekByKey("ArrowLeft", 30000, 60000)).toBe(25000);
    expect(seekByKey("ArrowRight", 30000, 60000)).toBe(35000);
  });

  it("clamps to the track bounds", () => {
    expect(seekByKey("ArrowLeft", 2000, 60000)).toBe(0);
    expect(seekByKey("ArrowRight", 59000, 60000)).toBe(60000);
  });

  it("steps past the end when the duration is unknown", () => {
    expect(seekByKey("ArrowRight", 10000, 0)).toBe(15000);
  });

  it("jumps to the start and end", () => {
    expect(seekByKey("Home", 30000, 60000)).toBe(0);
    expect(seekByKey("End", 30000, 60000)).toBe(60000);
    expect(seekByKey("End", 30000, 0)).toBeNull();
  });

  it("steps by a tenth of the duration on Page keys", () => {
    expect(seekByKey("PageUp", 30000, 60000)).toBe(36000);
    expect(seekByKey("PageDown", 30000, 60000)).toBe(24000);
    expect(seekByKey("PageUp", 30000, 0)).toBe(40000);
  });

  it("ignores other keys", () => {
    expect(seekByKey("Home", 30000, 0)).toBe(0);
    expect(seekByKey("x", 30000, 60000)).toBeNull();
  });
});
