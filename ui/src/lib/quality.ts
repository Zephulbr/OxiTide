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

/// The streaming tiers, lowest first, with what each one serves (the player
/// bar's quality menu).
export const QUALITY_TIERS: readonly { value: Quality; label: string; hint: string }[] = [
  { value: "low", label: "Low", hint: "HE-AAC 96 kbps" },
  { value: "high", label: "High", hint: "AAC 320 kbps" },
  { value: "lossless", label: "Lossless", hint: "CD quality, 16-bit / 44.1 kHz" },
  { value: "max", label: "Max (Hi-Res)", hint: "Highest available, up to 24-bit" },
];

export interface QualityPlate {
  /// `MAX` / `LOSSLESS` / `AAC` (or the preferred tier before a stream opens).
  tier: string;
  /// Detailed mode only: "24-bit · 96 kHz" for lossless, "320 kbps" for AAC.
  spec: string | null;
  max: boolean;
}

/** The player bar's quality plate: the tier word, plus the stream's format in
 *  detailed mode (the bitrate of a lossless stream lives in the menu). */
export function qualityPlate(
  meta: StreamMeta | null | undefined,
  mode: QualityDisplay,
  preferred: Quality,
): QualityPlate {
  const tier = meta ? compactLabel(meta.quality ?? preferred) : preferredLabel(preferred);
  const max = tier === "MAX";
  if (!meta || mode === "compact") return { tier, spec: null, max };
  if (tier === "AAC") {
    return { tier, spec: meta.bitrate_kbps ? `${meta.bitrate_kbps} kbps` : null, max };
  }
  if (meta.bit_depth <= 0 || meta.sample_rate <= 0) return { tier, spec: null, max };
  return { tier, spec: `${meta.bit_depth}-bit · ${formatRate(meta.sample_rate)}`, max };
}

export interface StreamReadout {
  /// The file type ("FLAC", "AAC"); null when the stream did not say.
  codec: string | null;
  /// "24-bit / 96 kHz · 2304 kbps"; null when nothing is known.
  spec: string | null;
}

/** The open stream for the quality menu's "Now playing" section. Null before
 *  a stream opens. */
export function streamReadout(meta: StreamMeta | null | undefined): StreamReadout | null {
  if (!meta) return null;
  const parts: string[] = [];
  if (meta.bit_depth > 0 && meta.sample_rate > 0) {
    parts.push(`${meta.bit_depth}-bit / ${formatRate(meta.sample_rate)}`);
  } else if (meta.sample_rate > 0) {
    parts.push(formatRate(meta.sample_rate));
  }
  if (meta.bitrate_kbps) parts.push(`${meta.bitrate_kbps} kbps`);
  return {
    codec: meta.codec ? meta.codec.toUpperCase() : null,
    spec: parts.length > 0 ? parts.join(" · ") : null,
  };
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
