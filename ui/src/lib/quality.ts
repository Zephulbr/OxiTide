import type { Quality, StreamMeta } from "./api";

export type QualityDisplay = "compact" | "detailed";

function preferredLabel(quality: Quality): string {
  switch (quality) {
    case "max":
      return "MAX";
    case "lossless":
      return "LOSSLESS";
    case "low":
      return "LOW";
    case "high":
      return "HIGH";
  }
}

function compactLabel(quality: Quality): string {
  switch (quality) {
    case "max":
      return "MAX";
    case "lossless":
      return "LOSSLESS";
    case "low":
    case "high":
      return "AAC";
  }
}

function formatRate(rate: number): string {
  const khz = rate / 1000;
  return `${Number.isInteger(khz) ? khz.toString() : khz.toFixed(1)} kHz`;
}

/** Compact (`MAX`/`LOSSLESS`/`AAC`) or detailed (`24-bit / 96 kHz / 2304 kbps`). */
export function formatQuality(
  meta: StreamMeta | null | undefined,
  mode: QualityDisplay,
  preferred: Quality,
): string {
  if (!meta) return preferredLabel(preferred);
  if (mode === "compact") return compactLabel(meta.quality ?? preferred);
  const bits = meta.bit_depth > 0 ? `${meta.bit_depth}-bit` : "—";
  const rate = meta.sample_rate > 0 ? formatRate(meta.sample_rate) : "—";
  const kbps = meta.bitrate_kbps ? `${meta.bitrate_kbps} kbps` : "—";
  return `${bits} / ${rate} / ${kbps}`;
}

/// TIDAL catalog `audioQuality` values mapped to the compact badge labels.
const TRACK_QUALITY_LABELS = {
  HI_RES_LOSSLESS: "MAX",
  HI_RES: "MAX",
  LOSSLESS: "LOSSLESS",
  HIGH: "AAC",
  LOW: "AAC",
} as const;

/** Badge label for a catalog track's `audioQuality`; null when absent or unrecognized. */
export function trackQualityLabel(
  audioQuality: string | null | undefined,
): "MAX" | "LOSSLESS" | "AAC" | null {
  if (!audioQuality) return null;
  return TRACK_QUALITY_LABELS[audioQuality as keyof typeof TRACK_QUALITY_LABELS] ?? null;
}
