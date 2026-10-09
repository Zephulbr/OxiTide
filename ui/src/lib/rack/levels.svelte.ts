import { listen as tauriListen } from "@tauri-apps/api/event";
import { api, LEVEL_FRAME_EVENT, type LevelFrame } from "../api";
import {
  bandFraction,
  CAP_FALL_PER_S,
  fallStep,
  FLOOR_DB,
  needleState,
  needleStep,
  peakStep,
  peakState,
  programmeDb,
  SPECTRUM_ATTACK_S,
  SPECTRUM_FALL_PER_S,
  vuPosition,
  type Needle,
  type PeakState,
} from "./ballistics";

/// Tap frames averaged (in power) for the VU input: ~100 ms, so the needle
/// follows programme level rather than every 33 ms reading.
const VU_WINDOW = 3;

export const BAND_COUNT = 28;

export interface LevelsSnapshot {
  /// Needle positions 0..1 (VU), L/R.
  needle: [number, number];
  /// Programme level and its hold, in meter dB (0 = reference), L/R.
  peakDb: [number, number];
  holdDb: [number, number];
  /// Spectrum bars and their falling caps, 0..1.
  bands: number[];
  caps: number[];
  /// The newest band levels, unsmoothed, 0..1 (the waterfall's input: the
  /// bars' slow fall would smear its history into stripes).
  raw: number[];
}

type Listen = (event: string, handler: (e: { payload: LevelFrame }) => void) => Promise<() => void>;

interface Deps {
  listen: Listen;
  setLevelMeter: (enabled: boolean) => Promise<void>;
  raf: (cb: (t: number) => void) => number;
  cancelRaf: (id: number) => void;
  visible: () => boolean;
  onVisibility: (cb: () => void) => void;
}

const browserDeps = (): Deps => ({
  listen: tauriListen as unknown as Listen,
  setLevelMeter: api.setLevelMeter,
  raf: (cb) => requestAnimationFrame(cb),
  cancelRaf: (id) => cancelAnimationFrame(id),
  visible: () => document.visibilityState === "visible",
  onVisibility: (cb) => document.addEventListener("visibilitychange", cb),
});

/// One shared feed for every Rack meter (RK-D7, RK-D14). Meters call `use()`
/// while mounted; the tap and the frame loop run only while at least one
/// meter is mounted and the window is visible.
export function createLevels(depsFactory: () => Deps = browserDeps) {
  const snapshot = $state<LevelsSnapshot>({
    needle: [-0.04, -0.04],
    peakDb: [FLOOR_DB, FLOOR_DB],
    holdDb: [FLOOR_DB, FLOOR_DB],
    bands: Array(BAND_COUNT).fill(0),
    caps: Array(BAND_COUNT).fill(0),
    raw: Array(BAND_COUNT).fill(0),
  });

  let deps: Deps | null = null;
  let users = 0;
  let running = false;
  let unlisten: (() => void) | null = null;
  let rafId: number | null = null;
  let lastT: number | null = null;
  let pending: LevelFrame | null = null;
  let vuTarget: [number, number] = [-0.04, -0.04];
  let bandTarget: number[] = Array(BAND_COUNT).fill(0);
  const recent: [number[], number[]] = [[], []];
  const needles: [Needle, Needle] = [needleState(), needleState()];
  const peaks: [PeakState, PeakState] = [peakState(), peakState()];

  function frame(t: number) {
    const dt = lastT === null ? 0 : Math.min(0.1, (t - lastT) / 1000);
    lastT = t;
    const f = pending;
    pending = null;
    if (f) {
      for (const ch of [0, 1] as const) {
        const rms = f.rms[ch] ?? 0;
        recent[ch].push(rms * rms);
        if (recent[ch].length > VU_WINDOW) recent[ch].shift();
        const power = recent[ch].reduce((a, b) => a + b, 0) / recent[ch].length;
        vuTarget[ch] = vuPosition(Math.sqrt(power));
      }
      bandTarget = f.bands.map(bandFraction);
      snapshot.raw = bandTarget;
    }
    for (const ch of [0, 1] as const) {
      needles[ch] = needleStep(needles[ch], vuTarget[ch], dt);
      snapshot.needle[ch] = needles[ch].x;
      peaks[ch] = peakStep(peaks[ch], f ? programmeDb(f.rms[ch] ?? 0) : null, dt, t);
      snapshot.peakDb[ch] = peaks[ch].db;
      snapshot.holdDb[ch] = peaks[ch].holdDb;
    }
    for (let i = 0; i < BAND_COUNT; i++) {
      const bar = fallStep(snapshot.bands[i], bandTarget[i] ?? 0, SPECTRUM_FALL_PER_S, dt, SPECTRUM_ATTACK_S);
      snapshot.bands[i] = bar;
      snapshot.caps[i] = fallStep(snapshot.caps[i], bar, CAP_FALL_PER_S, dt);
    }
    rafId = deps!.raf(frame);
  }

  async function start() {
    if (running || !deps) return;
    running = true;
    lastT = null;
    const off = await deps.listen(LEVEL_FRAME_EVENT, (e) => (pending = e.payload));
    if (!running) {
      off();
      return;
    }
    unlisten = off;
    deps.setLevelMeter(true).catch(() => {});
    rafId = deps.raf(frame);
  }

  function stop() {
    if (!running || !deps) return;
    running = false;
    unlisten?.();
    unlisten = null;
    if (rafId !== null) deps.cancelRaf(rafId);
    rafId = null;
    deps.setLevelMeter(false).catch(() => {});
  }

  function sync() {
    if (!deps) return;
    if (users > 0 && deps.visible()) void start();
    else stop();
  }

  return {
    get snapshot() { return snapshot; },
    /// Register a meter; returns the release function (use from `$effect`).
    use(): () => void {
      if (!deps) {
        deps = depsFactory();
        deps.onVisibility(sync);
      }
      users++;
      sync();
      return () => {
        users = Math.max(0, users - 1);
        sync();
      };
    },
  };
}

export type Levels = ReturnType<typeof createLevels>;

export const levels = createLevels();
