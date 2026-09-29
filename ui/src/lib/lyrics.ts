import type { LyricLine } from "./api";

/// How early a line highlights, compensating for position-event cadence.
export const LYRICS_LOOKAHEAD_MS = 150;

/// Estimated pace cap, in ms per word: a line never drags across a long
/// instrumental gap. No minimum — dense lines must stay inside their gap so
/// every word still flips before the next line takes over.
const WORD_PACE_MAX_MS = 700;
/// Fallback pace for the last line (and for a next stamp that is not after
/// this line), where the end is unknown or unusable.
const WORD_PACE_FALLBACK_MS = 400;

export interface TimedWord {
  time_ms: number;
  text: string;
}

/// Word timing for the karaoke fill: enhanced-LRC stamps when the parser found
/// them, otherwise estimated by distributing the line's span (to the next timed
/// line) across its words, weighted by word length. Text tokens keep their
/// trailing whitespace so rendering preserves the original spacing.
export function lineWords(line: LyricLine, nextStartMs: number | null): TimedWord[] {
  if (line.words && line.words.length > 0) return line.words;
  if (line.time_ms === null) return [];
  const tokens = line.text.match(/\S+\s*/g);
  if (!tokens) return [];
  const weights = tokens.map((token) => token.trim().length || 1);
  const total = weights.reduce((sum, weight) => sum + weight, 0);
  let gap =
    nextStartMs !== null ? nextStartMs - line.time_ms : tokens.length * WORD_PACE_FALLBACK_MS;
  if (gap <= 0) gap = tokens.length * WORD_PACE_FALLBACK_MS;
  gap = Math.min(gap, tokens.length * WORD_PACE_MAX_MS);
  const perWeight = gap / total;
  const words: TimedWord[] = [];
  let cumulative = 0;
  for (let i = 0; i < tokens.length; i++) {
    words.push({ time_ms: line.time_ms + Math.round(cumulative * perWeight), text: tokens[i] });
    cumulative += weights[i];
  }
  return words;
}

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
