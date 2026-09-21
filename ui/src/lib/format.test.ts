import { describe, it, expect } from "vitest";
import { formatDuration } from "./format";

describe("formatDuration", () => {
  it("formats minutes and seconds with padding", () => {
    expect(formatDuration(276)).toBe("4:36");
    expect(formatDuration(65)).toBe("1:05");
  });
  it("never returns negative", () => {
    expect(formatDuration(-10)).toBe("0:00");
  });
});
