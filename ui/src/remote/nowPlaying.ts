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

/// Movement (px) before a touch on the cover counts as a volume swipe, so a
/// plain tap never changes the volume.
export const SWIPE_THRESHOLD_PX = 6;
/// Arrow-key / step size for the cover volume control.
export const VOLUME_STEP = 0.05;

/// Volume for a vertical drag on the cover: dragging up by the cover's full
/// height adds 100%. Clamped to 0–1, rounded to whole percent.
export function volumeFromDrag(start: number, deltaY: number, height: number): number {
  const change = height > 0 ? -deltaY / height : 0;
  return Math.round(Math.min(1, Math.max(0, start + change)) * 100) / 100;
}

export function stepVolume(volume: number, direction: 1 | -1): number {
  return Math.round(Math.min(1, Math.max(0, volume + direction * VOLUME_STEP)) * 100) / 100;
}

/// `m:ss` from milliseconds.
export function clock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, "0")}`;
}
