import type { RemoteState, RepeatMode } from "../lib/api";

/// The sleep-timer fade length on the desktop (`SLEEP_FADE_MS`).
export const REMOTE_FADE_MS = 5000;

/// Off → all → one → off, like the desktop transport.
export function nextRepeat(mode: RepeatMode): RepeatMode {
  return mode === "off" ? "all" : mode === "all" ? "one" : "off";
}

/// Compact badge plus optional detail (`24-bit / 96 kHz`).
export function qualityBadges(state: RemoteState): { label: string; detail: string } | null {
  const quality = state.current?.audioQuality;
  if (!state.current) return null;
  const label = quality === "max" ? "MAX" : quality === "lossless" ? "LOSSLESS" : quality ? "AAC" : "";
  const stream = state.stream;
  let detail = "";
  if (stream && stream.sampleRate > 0) {
    const khz = stream.sampleRate / 1000;
    const rate = `${Number.isInteger(khz) ? khz : khz.toFixed(1)} kHz`;
    detail = stream.bitDepth > 0 ? `${stream.bitDepth}-bit / ${rate}` : rate;
  }
  if (!label && !detail) return null;
  return { label, detail };
}

/// `m:ss` from milliseconds.
export function clock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, "0")}`;
}
