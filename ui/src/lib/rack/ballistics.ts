/// Meter ballistics for the Hi-Fi Rack (DESIGN §5.9, spec §6.7). Pure; the
/// levels store calls these once per animation frame.

export const FLOOR_DB = -96;
/// VU integration time constant: ~99% of a step in 300 ms.
export const VU_TAU_S = 0.065;
/// 0 VU = −18 dBFS RMS (EBU alignment).
export const VU_REFERENCE_DBFS = -18;
export const PEAK_FALL_DB_PER_S = 24;
export const PEAK_HOLD_MS = 1500;
export const PEAK_HOLD_FALL_DB_PER_S = 12;
export const SPECTRUM_FALL_PER_S = 1.4;
export const CAP_FALL_PER_S = 0.35;
export const SPECTRUM_FLOOR_DB = -60;

export function dbfs(linear: number): number {
  return linear > 0 ? Math.max(FLOOR_DB, 20 * Math.log10(linear)) : FLOOR_DB;
}

/// One VU integration step on linear RMS amplitude.
export function vuStep(current: number, target: number, dt: number): number {
  if (dt <= 0) return current;
  return current + (target - current) * (1 - Math.exp(-dt / VU_TAU_S));
}

/// Needle position 0..1 across the −20…+3 VU scale (slightly past at both
/// ends, like a pinned needle).
export function vuPosition(rms: number): number {
  const vuDb = dbfs(rms) - VU_REFERENCE_DBFS;
  const pos = (10 ** (vuDb / 20) - 0.1) / (10 ** (3 / 20) - 0.1);
  return Math.min(1.04, Math.max(-0.04, pos));
}

export function needleAngle(position: number): number {
  return -46 + position * 92;
}

export interface PeakState {
  db: number;
  holdDb: number;
  holdAt: number;
}

export function peakState(): PeakState {
  return { db: FLOOR_DB, holdDb: FLOOR_DB, holdAt: 0 };
}

/// Instant attack, 24 dB/s fall, 1.5 s hold then a 12 dB/s fall.
/// `input` is the newest reading (or null when none arrived this frame).
export function peakStep(state: PeakState, input: number | null, dt: number, now: number): PeakState {
  let db = Math.max(FLOOR_DB, state.db - PEAK_FALL_DB_PER_S * dt);
  if (input !== null && input > db) db = input;
  let { holdDb, holdAt } = state;
  if (db >= holdDb) {
    holdDb = db;
    holdAt = now;
  } else if (now - holdAt > PEAK_HOLD_MS) {
    holdDb = Math.max(db, holdDb - PEAK_HOLD_FALL_DB_PER_S * dt);
  }
  return { db, holdDb, holdAt };
}

/// Position of a dBFS value on the −40…+3 peak scale.
export function peakFraction(db: number, min = -40, max = 3): number {
  return Math.min(1, Math.max(0, (db - min) / (max - min)));
}

/// Band level 0..1 on the −60…0 dB spectrum scale.
export function bandFraction(db: number): number {
  return Math.min(1, Math.max(0, (db - SPECTRUM_FLOOR_DB) / -SPECTRUM_FLOOR_DB));
}

/// Rise instantly, fall at `rate` per second.
export function fallStep(current: number, target: number, rate: number, dt: number): number {
  return target >= current ? target : Math.max(target, current - rate * dt);
}

/// LED ladder thresholds (dBFS), DESIGN §5.9.
export const LED_STEPS = [-40, -30, -24, -20, -17, -14, -12, -10, -8, -7, -6, -5, -4, -3, -2, -1, 0, 1, 2, 3];

/// Number of lit LEDs for a level.
export function ledCount(db: number): number {
  let n = 0;
  for (const step of LED_STEPS) if (db >= step) n++;
  return n;
}

/// Reduce the tap's 28 bands to `n` (the amp's 20) by max-pooling.
export function poolBands(bands: number[], n: number): number[] {
  if (bands.length <= n) return bands.slice();
  return Array.from({ length: n }, (_, i) => {
    const start = Math.floor((i * bands.length) / n);
    const end = Math.max(start + 1, Math.floor(((i + 1) * bands.length) / n));
    return Math.max(...bands.slice(start, end));
  });
}
