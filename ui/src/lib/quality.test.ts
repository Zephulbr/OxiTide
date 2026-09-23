import { describe, it, expect } from "vitest";
import { formatQuality } from "./quality";
import type { StreamMeta } from "./api";

function meta(over: Partial<StreamMeta> = {}): StreamMeta {
  return {
    quality: "lossless",
    codec: "flac",
    sample_rate: 44100,
    bit_depth: 16,
    channels: 2,
    bitrate_kbps: 1000,
    ...over,
  };
}

describe("formatQuality compact", () => {
  it("labels the served quality", () => {
    expect(formatQuality(meta({ quality: "max" }), "compact", "lossless")).toBe("MAX");
    expect(formatQuality(meta({ quality: "lossless" }), "compact", "max")).toBe("LOSSLESS");
    expect(formatQuality(meta({ quality: "high" }), "compact", "max")).toBe("AAC");
    expect(formatQuality(meta({ quality: "low" }), "compact", "max")).toBe("AAC");
  });

  it("falls back to the preferred quality before streaming starts", () => {
    expect(formatQuality(null, "compact", "max")).toBe("MAX");
    expect(formatQuality(undefined, "compact", "high")).toBe("HIGH");
    expect(formatQuality(meta({ quality: null }), "compact", "lossless")).toBe("LOSSLESS");
  });
});

describe("formatQuality detailed", () => {
  it("shows the exact format", () => {
    expect(
      formatQuality(meta({ bit_depth: 24, sample_rate: 96000, bitrate_kbps: 2304 }), "detailed", "max"),
    ).toBe("24-bit / 96 kHz / 2304 kbps");
  });

  it("dashes missing segments", () => {
    expect(formatQuality(meta({ bitrate_kbps: null }), "detailed", "max")).toBe(
      "16-bit / 44.1 kHz / —",
    );
  });

  it("falls back to the preferred quality without stream metadata", () => {
    expect(formatQuality(null, "detailed", "lossless")).toBe("LOSSLESS");
  });
});
