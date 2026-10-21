import { describe, it, expect } from "vitest";
import { formatQuality, qualityPlate, streamReadout, trackQualityLabel } from "./quality";
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
    expect(formatQuality(null, "compact", "low")).toBe("LOW");
    expect(formatQuality(undefined, "compact", "low")).toBe("LOW");
    expect(formatQuality(meta({ quality: null }), "compact", "lossless")).toBe("LOSSLESS");
  });
});

describe("formatQuality detailed", () => {
  it("shows the exact format", () => {
    expect(
      formatQuality(meta({ bit_depth: 24, sample_rate: 96000, bitrate_kbps: 2304 }), "detailed", "max"),
    ).toBe("24-bit / 96 kHz / 2304 kbps");
  });

  it("shows the served Low bitrate without inventing a bit depth", () => {
    expect(
      formatQuality(
        meta({ quality: "low", codec: "aac", bit_depth: 0, bitrate_kbps: 96 }),
        "detailed",
        "max",
      ),
    ).toBe("— / 44.1 kHz / 96 kbps");
  });

  it("dashes missing segments", () => {
    expect(formatQuality(meta({ bitrate_kbps: null }), "detailed", "max")).toBe(
      "16-bit / 44.1 kHz / —",
    );
  });

  it("falls back to the preferred quality without stream metadata", () => {
    expect(formatQuality(null, "detailed", "lossless")).toBe("LOSSLESS");
    expect(formatQuality(null, "detailed", "low")).toBe("LOW");
    expect(formatQuality(undefined, "detailed", "low")).toBe("LOW");
  });
});

describe("trackQualityLabel", () => {
  it("maps TIDAL catalog qualities to compact labels", () => {
    expect(trackQualityLabel("HI_RES_LOSSLESS")).toBe("MAX");
    expect(trackQualityLabel("HI_RES")).toBe("MAX");
    expect(trackQualityLabel("LOSSLESS")).toBe("LOSSLESS");
    expect(trackQualityLabel("HIGH")).toBe("AAC");
    expect(trackQualityLabel("LOW")).toBe("AAC");
  });

  it("returns null for missing or unknown qualities", () => {
    expect(trackQualityLabel(null)).toBeNull();
    expect(trackQualityLabel(undefined)).toBeNull();
    expect(trackQualityLabel("")).toBeNull();
    expect(trackQualityLabel("DOLBY_ATMOS")).toBeNull();
  });
});

describe("qualityPlate", () => {
  it("shows the tier alone in compact mode", () => {
    expect(qualityPlate(meta({ quality: "max", bit_depth: 24 }), "compact", "max")).toEqual({
      tier: "MAX",
      spec: null,
      max: true,
    });
  });

  it("adds bit depth and rate for lossless streams, leaving the bitrate to the menu", () => {
    expect(qualityPlate(meta(), "detailed", "max")).toEqual({
      tier: "LOSSLESS",
      spec: "16-bit · 44.1 kHz",
      max: false,
    });
  });

  it("shows only the bitrate for AAC", () => {
    const aac = meta({ quality: "high", codec: "aac", bit_depth: 0, bitrate_kbps: 320 });
    expect(qualityPlate(aac, "detailed", "max").spec).toBe("320 kbps");
  });

  it("shows the preferred tier before a stream opens", () => {
    expect(qualityPlate(null, "detailed", "high")).toEqual({ tier: "HIGH", spec: null, max: false });
  });
});

describe("streamReadout", () => {
  it("spells out the open stream", () => {
    expect(streamReadout(meta({ bit_depth: 24, sample_rate: 96000, bitrate_kbps: 2304 }))).toEqual({
      codec: "FLAC",
      spec: "24-bit / 96 kHz · 2304 kbps",
    });
  });

  it("skips what is unknown", () => {
    expect(streamReadout(meta({ codec: "aac", bit_depth: 0, bitrate_kbps: null }))).toEqual({
      codec: "AAC",
      spec: "44.1 kHz",
    });
    expect(streamReadout(meta({ codec: null, sample_rate: 0, bit_depth: 0, bitrate_kbps: null }))).toEqual({
      codec: null,
      spec: null,
    });
    expect(streamReadout(null)).toBeNull();
  });
});