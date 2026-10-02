import { describe, it, expect } from "vitest";
import { formatDuration, formatTotalDuration } from "./format";

describe("formatDuration", () => {
  it("formats minutes and seconds with padding", () => {
    expect(formatDuration(276)).toBe("4:36");
    expect(formatDuration(65)).toBe("1:05");
  });
  it("never returns negative", () => {
    expect(formatDuration(-10)).toBe("0:00");
  });
});

describe("formatTotalDuration", () => {
  it("adds an hours segment at one hour and above", () => {
    expect(formatTotalDuration(36968)).toBe("10:16:08");
    expect(formatTotalDuration(3600)).toBe("1:00:00");
  });
  it("keeps the minute form below an hour", () => {
    expect(formatTotalDuration(276)).toBe("4:36");
    expect(formatTotalDuration(65)).toBe("1:05");
  });
  it("never returns negative", () => {
    expect(formatTotalDuration(-10)).toBe("0:00");
  });
});
