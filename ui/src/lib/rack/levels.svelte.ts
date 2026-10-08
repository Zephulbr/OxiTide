import { listen as tauriListen } from "@tauri-apps/api/event";
import { api, LEVEL_FRAME_EVENT, type LevelFrame } from "../api";
import {
  bandFraction,
  CAP_FALL_PER_S,
  dbfs,
  fallStep,
  FLOOR_DB,
  peakStep,
  peakState,
  SPECTRUM_FALL_PER_S,
  vuPosition,
  vuStep,
  type PeakState,
} from "./ballistics";

export const BAND_COUNT = 28;

export interface LevelsSnapshot {
  /// Needle positions 0..1 (VU), L/R.
  needle: [number, number];
  /// Peak level and peak-hold level in dBFS, L/R.
  peakDb: [number, number];
  holdDb: [number, number];
  /// Spectrum bars and their falling caps, 0..1.
  bands: number[];
  caps: number[];
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
  });

  let deps: Deps | null = null;
  let users = 0;
  let running = false;
  let unlisten: (() => void) | null = null;
  let rafId: number | null = null;
  let lastT: number | null = null;
  let pending: LevelFrame | null = null;
  let rmsTarget: [number, number] = [0, 0];
  let bandTarget: number[] = Array(BAND_COUNT).fill(0);
  const vu: [number, number] = [0, 0];
  const peaks: [PeakState, PeakState] = [peakState(), peakState()];

  function frame(t: number) {
    const dt = lastT === null ? 0 : Math.min(0.1, (t - lastT) / 1000);
    lastT = t;
    const f = pending;
    pending = null;
    if (f) {
      rmsTarget = [f.rms[0] ?? 0, f.rms[1] ?? 0];
      bandTarget = f.bands.map(bandFraction);
    }
    for (const ch of [0, 1] as const) {
      vu[ch] = vuStep(vu[ch], rmsTarget[ch], dt);
      snapshot.needle[ch] = vuPosition(vu[ch]);
      peaks[ch] = peakStep(peaks[ch], f ? dbfs(f.peak[ch] ?? 0) : null, dt, t);
      snapshot.peakDb[ch] = peaks[ch].db;
      snapshot.holdDb[ch] = peaks[ch].holdDb;
    }
    for (let i = 0; i < BAND_COUNT; i++) {
      const bar = fallStep(snapshot.bands[i], bandTarget[i] ?? 0, SPECTRUM_FALL_PER_S, dt);
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
