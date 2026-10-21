import type { BeatClock } from "./api";

/// Residual trim for output latency the driver does not report (spec CJ-D9);
/// the measured lead arrives with every position event.
export const BEAT_TRIM_MS = 0;
/// Drift under this is left alone: position events arrive every 500 ms and a
/// correction each time would make the cat skip a frame (CJ-D10).
export const SNAP_MS = 15;
/// Larger drift is eased out over this long.
export const EASE_MS = 200;
/// Drift past this is a jump (an unflagged seek), applied at once.
export const JUMP_MS = 1000;

/// The heard track position between the engine's 500 ms position events.
export interface PositionClock {
  /// An engine position. `jump`: a seek or a new track, applied at once.
  sample(positionMs: number, leadMs: number, nowMs: number, jump?: boolean): void;
  /// Freeze (paused) or resume extrapolating at `nowMs`.
  setPlaying(playing: boolean, nowMs: number): void;
  /// The heard position at `nowMs`, or null before the first sample.
  heardAt(nowMs: number): number | null;
}

export function createPositionClock(): PositionClock {
  let has = false;
  let playing = false;
  let base = 0;
  let baseAt = 0;
  let offset = 0;
  let easeStart = 0;

  function at(nowMs: number): number {
    const linear = base + (playing ? nowMs - baseAt : 0);
    if (offset === 0) return linear;
    const t = Math.min(1, Math.max(0, (nowMs - easeStart) / EASE_MS));
    return linear + offset * t;
  }

  function anchor(value: number, nowMs: number) {
    base = value;
    baseAt = nowMs;
    offset = 0;
  }

  return {
    sample(positionMs, leadMs, nowMs, jump = false) {
      const target = positionMs - leadMs - BEAT_TRIM_MS;
      if (!has || jump) {
        has = true;
        anchor(target, nowMs);
        return;
      }
      const current = at(nowMs);
      const drift = target - current;
      if (Math.abs(drift) >= JUMP_MS) {
        anchor(target, nowMs);
      } else if (Math.abs(drift) >= SNAP_MS) {
        anchor(current, nowMs);
        offset = drift;
        easeStart = nowMs;
      }
    },
    setPlaying(next, nowMs) {
      if (next === playing) return;
      anchor(at(nowMs), nowMs);
      playing = next;
    },
    heardAt(nowMs) {
      return has ? at(nowMs) : null;
    },
  };
}

/// Whole beats from the clock's anchor to `positionMs` (negative before it).
export function beatIndex(clock: BeatClock, positionMs: number): number {
  if (!(clock.period_ms > 0)) return 0;
  return Math.floor((positionMs - clock.anchor_ms) / clock.period_ms);
}

/// How far into the current beat `positionMs` is, 0 (on the beat) to <1.
export function beatPhase(clock: BeatClock, positionMs: number): number {
  if (!(clock.period_ms > 0)) return 0;
  const beats = (positionMs - clock.anchor_ms) / clock.period_ms;
  return beats - Math.floor(beats);
}
