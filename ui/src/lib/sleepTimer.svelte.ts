/// How long the sleep timer fades before pausing (spec §6.2).
export const SLEEP_FADE_MS = 5000;

export type SleepTimerMode = "off" | "duration" | "end-of-track";

export interface SleepTimer {
  readonly mode: SleepTimerMode;
  /// The armed duration in minutes (menu selection state); null otherwise.
  readonly totalMinutes: number | null;
  /// Milliseconds left on a duration timer; null for other modes.
  readonly remainingMs: number | null;
  armMinutes(minutes: number): void;
  armEndOfTrack(): void;
  cancel(): void;
  /// Fed from the player's 500 ms position ticks; fires end-of-track mode when
  /// five seconds remain, and disarms when the queue is over.
  observe(positionMs: number, durationMs: number, playing: boolean, queueLength: number): void;
}

/// The sleep timer is in-memory only (QoL-D4): a restart disarms it. `fire`
/// is called exactly once per armed timer with the fade length.
export function createSleepTimer(fire: (fadeMs: number) => void): SleepTimer {
  let mode = $state<SleepTimerMode>("off");
  let totalMinutes = $state<number | null>(null);
  let nowMs = $state(Date.now());
  let endsAt = 0;
  let ticker: ReturnType<typeof setInterval> | null = null;

  function stopTicker() {
    if (ticker !== null) {
      clearInterval(ticker);
      ticker = null;
    }
  }

  function fireOnce() {
    if (mode === "off") return;
    mode = "off";
    totalMinutes = null;
    stopTicker();
    fire(SLEEP_FADE_MS);
  }

  function armMinutes(minutes: number) {
    const rounded = Math.round(minutes);
    if (!Number.isFinite(rounded) || rounded < 1) return;
    totalMinutes = rounded;
    nowMs = Date.now();
    endsAt = nowMs + rounded * 60_000;
    mode = "duration";
    stopTicker();
    ticker = setInterval(() => {
      nowMs = Date.now();
      if (mode === "duration" && nowMs >= endsAt) fireOnce();
    }, 500);
  }

  function armEndOfTrack() {
    stopTicker();
    totalMinutes = null;
    mode = "end-of-track";
  }

  function cancel() {
    stopTicker();
    mode = "off";
    totalMinutes = null;
  }

  function observe(
    positionMs: number,
    durationMs: number,
    playing: boolean,
    queueLength: number,
  ) {
    if (mode === "off") return;
    if (queueLength === 0) {
      cancel();
      return;
    }
    if (!playing && durationMs > 0 && positionMs + 500 >= durationMs) {
      // The queue reached its natural end; nothing left to sleep through. The
      // last position tick can lag the duration by up to 500 ms.
      cancel();
      return;
    }
    if (mode !== "end-of-track" || !playing || durationMs === 0) return;
    const remaining = durationMs - positionMs;
    if (remaining > 0 && remaining <= SLEEP_FADE_MS) fireOnce();
  }

  return {
    get mode() {
      return mode;
    },
    get totalMinutes() {
      return totalMinutes;
    },
    get remainingMs() {
      return mode === "duration" ? Math.max(0, endsAt - nowMs) : null;
    },
    armMinutes,
    armEndOfTrack,
    cancel,
    observe,
  };
}
