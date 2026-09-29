import type { LyricLine } from "./api";

/// How early a line highlights, compensating for position-event cadence.
export const LYRICS_LOOKAHEAD_MS = 150;

/// Index of the line that should be highlighted at `positionMs`, or -1 when no
/// line is active yet. Only timed, non-blank lines can be active; timestamps may
/// arrive out of order, so the latest qualifying line by time wins.
export function activeLine(
  lines: LyricLine[],
  positionMs: number,
  lookaheadMs = LYRICS_LOOKAHEAD_MS,
): number {
  const threshold = positionMs + lookaheadMs;
  let active = -1;
  let best = -Infinity;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.time_ms === null || line.text.trim() === "") continue;
    if (line.time_ms <= threshold && line.time_ms >= best) {
      best = line.time_ms;
      active = i;
    }
  }
  return active;
}
