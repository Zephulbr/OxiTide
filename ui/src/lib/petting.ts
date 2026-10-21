/// Petting the oneko cats (spec 2026-10-21-oxitide-catjam-kuroneko-design.md,
/// KN-D4): a rub is the cursor going back and forth over a cat, at least
/// `reversals` horizontal turns within `windowMs`. A cursor that just passes
/// over her never turns, so it never counts.
export const RUB_REVERSALS = 3;
export const RUB_WINDOW_MS = 1200;
/// Smaller moves are jitter, not a direction.
const MIN_STEP_PX = 2;

export interface RubDetector {
  /// One pointer sample; true when it completed a rub (once per rub).
  feed(x: number, timeMs: number, inside: boolean): boolean;
}

export function createRubDetector({
  reversals = RUB_REVERSALS,
  windowMs = RUB_WINDOW_MS,
}: { reversals?: number; windowMs?: number } = {}): RubDetector {
  let lastX: number | null = null;
  let direction = 0;
  let turns: number[] = [];

  function reset() {
    lastX = null;
    direction = 0;
    turns = [];
  }

  return {
    feed(x, timeMs, inside) {
      if (!inside) {
        reset();
        return false;
      }
      if (lastX === null) {
        lastX = x;
        return false;
      }
      const dx = x - lastX;
      if (Math.abs(dx) < MIN_STEP_PX) return false;
      lastX = x;
      const next = Math.sign(dx);
      if (direction !== 0 && next !== direction) turns.push(timeMs);
      direction = next;
      turns = turns.filter((at) => timeMs - at <= windowMs);
      if (turns.length < reversals) return false;
      turns = [];
      return true;
    },
  };
}
