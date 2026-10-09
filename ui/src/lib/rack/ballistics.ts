/// Meter ballistics for the Hi-Fi Rack (DESIGN §5.9, spec §6.7). Pure; the
/// levels store calls these once per animation frame.
///
/// Calibration: real hi-fi meters are aligned to a programme reference, not
/// to digital full scale — otherwise modern masters (short-term RMS around
/// −8…−12 dBFS, sample peaks at −1…−3 dBFS) pin every meter at the top.
/// All meters read programme level relative to REFERENCE_DBFS RMS.

export const FLOOR_DB = -96;
/// 0 VU / 0 dB on the bars and LEDs.
export const REFERENCE_DBFS = -10;

/// VU needle: a damped mass-spring (2.0 Hz, ζ 0.8) — about 300 ms to settle,
/// ~1.5 % overshoot, like the IEC 60268-17 movement.
export const NEEDLE_HZ = 2.0;
export const NEEDLE_DAMPING = 0.8;

/// Bar/LED programme meters (PPM-like): fast rise, slow fall, peak hold.
export const PROGRAMME_ATTACK_S = 0.01;
export const PROGRAMME_FALL_DB_PER_S = 13; // 20 dB in ~1.5 s
export const PEAK_HOLD_MS = 1500;
export const PEAK_HOLD_FALL_DB_PER_S = 10;

/// Spectrum: smoothed rise, slow fall, slower caps.
export const SPECTRUM_ATTACK_S = 0.05;
export const SPECTRUM_FALL_PER_S = 0.9;
export const CAP_FALL_PER_S = 0.25;
export const SPECTRUM_FLOOR_DB = -60;

export function dbfs(linear: number): number {
  return linear > 0 ? Math.max(FLOOR_DB, 20 * Math.log10(linear)) : FLOOR_DB;
}

/// Programme level in meter dB (0 = reference) for a linear RMS value.
export function programmeDb(rms: number): number {
  return dbfs(rms) - REFERENCE_DBFS;
}

/// Needle position 0..1 across the −20…+3 VU scale (slightly past at both
/// ends, like a pinned needle).
export function vuPosition(rms: number): number {
  const vuDb = programmeDb(rms);
  const pos = (10 ** (vuDb / 20) - 0.1) / (10 ** (3 / 20) - 0.1);
  return Math.min(1.04, Math.max(-0.04, pos));
}

export function needleAngle(position: number): number {
  return -46 + position * 92;
}

export interface Needle {
  x: number;
  v: number;
}

export function needleState(): Needle {
  return { x: -0.04, v: 0 };
}

/// Advance the needle toward `target` (a VU position) by `dt` seconds.
/// Sub-stepped so long frames stay stable.
export function needleStep(state: Needle, target: number, dt: number): Needle {
  const w = 2 * Math.PI * NEEDLE_HZ;
  let { x, v } = state;
  let left = Math.max(0, dt);
  while (left > 0) {
    const h = Math.min(left, 1 / 240);
    const a = w * w * (target - x) - 2 * NEEDLE_DAMPING * w * v;
    v += a * h;
    x += v * h;
    left -= h;
  }
  // The pin stops the needle at both ends.
  if (x < -0.04) {
    x = -0.04;
    v = Math.max(0, v);
  } else if (x > 1.04) {
    x = 1.04;
    v = Math.min(0, v);
  }
  return { x, v };
}

export interface PeakState {
  db: number;
  holdDb: number;
  holdAt: number;
}

export function peakState(): PeakState {
  return { db: FLOOR_DB, holdDb: FLOOR_DB, holdAt: 0 };
}

/// Programme meter: rises toward the input with a 10 ms time constant, falls
/// at 13 dB/s, and holds its highest reading for 1.5 s before it falls.
/// `input` is the newest level in meter dB (or null when none arrived).
export function peakStep(state: PeakState, input: number | null, dt: number, now: number): PeakState {
  let db = Math.max(FLOOR_DB, state.db - PROGRAMME_FALL_DB_PER_S * dt);
  if (input !== null && input > db) {
    const k = 1 - Math.exp(-dt / PROGRAMME_ATTACK_S);
    db = db + (input - db) * (dt > 0 ? k : 1);
  }
  let { holdDb, holdAt } = state;
  if (db >= holdDb) {
    holdDb = db;
    holdAt = now;
  } else if (now - holdAt > PEAK_HOLD_MS) {
    holdDb = Math.max(db, holdDb - PEAK_HOLD_FALL_DB_PER_S * dt);
  }
  return { db, holdDb, holdAt };
}

/// Position of a meter-dB value on the −40…+3 scale.
export function peakFraction(db: number, min = -40, max = 3): number {
  return Math.min(1, Math.max(0, (db - min) / (max - min)));
}

/// Band level 0..1 on the −60…0 dB spectrum scale.
export function bandFraction(db: number): number {
  return Math.min(1, Math.max(0, (db - SPECTRUM_FLOOR_DB) / -SPECTRUM_FLOOR_DB));
}

/// Fall at `rate` per second; rise instantly, or smoothed by `attackS`.
export function fallStep(current: number, target: number, rate: number, dt: number, attackS = 0): number {
  if (target >= current) {
    if (attackS <= 0 || dt <= 0) return target;
    return current + (target - current) * (1 - Math.exp(-dt / attackS));
  }
  return Math.max(target, current - rate * dt);
}

/// LED ladder thresholds (meter dB), DESIGN §5.9.
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
