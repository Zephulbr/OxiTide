import { listen } from "@tauri-apps/api/event";
import { api, type EngineEvent, type PlaybackState, type Quality, type QueueItem, type RepeatMode, type StreamMeta } from "./api";
import type { Toasts } from "./toasts.svelte";

export interface PlayerState {
  playing: boolean;
  buffering: boolean;
  current: QueueItem | null;
  current_index: number | null;
  position_ms: number;
  duration_ms: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  preferred_quality: Quality;
  output_mode: "shared" | "exclusive";
  bit_perfect: boolean;
  bit_perfect_notice: string | null;
  dsp_active: boolean;
  dsp_stages: string[];
  crossfade_ms: number;
  queue: QueueItem[];
  stream: StreamMeta | null;
  error: string;
  /// Engine error code ("output", "playback", "auth"); drives how the error
  /// clears: output failures wait for the engine to report recovery.
  error_code: string;
}

export function emptyState(): PlayerState {
  return {
    playing: false, buffering: false, current: null, current_index: null, position_ms: 0,
    duration_ms: 0, volume: 1, shuffle: false, repeat: "off",
    preferred_quality: "lossless", queue: [], stream: null, error: "",
    error_code: "", output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
    dsp_active: false, dsp_stages: [], crossfade_ms: 0,
  };
}

/// Apply an engine event to `state` in place. The returned object is the same
/// reference, so Svelte invalidates only the properties that changed: position
/// events arrive every 500 ms and a whole-state replacement re-ran every reader.
export function applyEvent(state: PlayerState, event: EngineEvent): PlayerState {
  switch (event.type) {
    case "state": {
      const { type, ...rest } = event;
      Object.assign(state, rest);
      return state;
    }
    case "position":
      state.position_ms = event.position_ms;
      state.duration_ms = event.duration_ms;
      return state;
    case "now_playing":
      state.error = "";
      state.error_code = "";
      return state;
    case "error":
      state.error = event.message;
      state.error_code = event.code;
      return state;
    default:
      return state;
  }
}

export function reduce(state: PlayerState, event: EngineEvent): PlayerState {
  return applyEvent({ ...state }, event);
}

/** Keeps the slider pinned to the requested position until the engine catches up. */
export function reconcileSeek(pending: number | null, event: EngineEvent): number | null {
  if (pending === null) return null;
  if (event.type === "position") {
    return Math.abs(event.position_ms - pending) <= 500 ? null : pending;
  }
  if (event.type === "error" || event.type === "now_playing") return null;
  return pending;
}

/** Volume to apply when toggling mute: silence when audible, restore when muted. */
export function muteTarget(volume: number, lastVolume: number): number {
  if (volume > 0) return 0;
  return lastVolume > 0 ? lastVolume : 0.5;
}

/** Speaker icon for a volume level: crossed when muted, one wave below half, two at/above half. */
export function volumeIcon(volume: number): "volume-x" | "volume-1" | "volume-2" {
  if (volume <= 0) return "volume-x";
  return volume < 0.5 ? "volume-1" : "volume-2";
}

export function createPlayer(toasts?: Toasts) {
  let state = $state<PlayerState>(emptyState());
  let pendingSeek = $state<number | null>(null);
  let lastVolume = 0.5;
  let errorToastId: number | null = null;

  function dismissErrorToast() {
    if (toasts !== undefined && errorToastId !== null) {
      toasts.dismiss(errorToastId);
      errorToastId = null;
    }
  }

  async function init() {
    await listen<EngineEvent>("playback_event", (event) => {
      applyEvent(state, event.payload);
      pendingSeek = reconcileSeek(pendingSeek, event.payload);
      if (toasts === undefined) return;
      if (event.payload.type === "error") {
        dismissErrorToast();
        errorToastId = toasts.show(event.payload.message, {
          severity: "error",
          action: { label: "Retry", run: () => void retry() },
        });
      } else if (event.payload.type === "now_playing") {
        dismissErrorToast();
      } else if (
        event.payload.type === "state" &&
        state.error_code === "output" &&
        state.error !== "" &&
        event.payload.bit_perfect_notice !== state.error
      ) {
        // The engine reopened the output and no longer reports the failure in
        // its notice; clear the error now so the recovered status can show.
        state.error = "";
        state.error_code = "";
        dismissErrorToast();
      }
    });
    try {
      const snapshot = await api.getPlaybackState();
      applyEvent(state, { type: "state", ...snapshot });
      if (snapshot.volume > 0) lastVolume = snapshot.volume;
    } catch {
      /* not logged in yet */
    }
  }

  function seek(ms: number) {
    const target = Math.max(0, Math.round(ms));
    pendingSeek = target;
    return api.seek(target);
  }

  async function retry() {
    await api.retry();
    dismissErrorToast();
    // The engine emits a State event, which does not clear the mirrored
    // error; non-output failures clear here once the command was accepted.
    // Output failures stay latched until the engine publishes a state whose
    // notice no longer reports them (the listener above), so the stale bar
    // notice can never flash between the command and the actual recovery.
    if (state.error_code === "output") return;
    state.error = "";
    state.error_code = "";
  }

  /// Rebuild the queue from a saved session: the engine loads the exact order
  /// paused at the saved position, so nothing plays until the user asks.
  async function restore(
    queue: QueueItem[],
    index: number,
    positionMs: number,
    shuffle: boolean,
    repeat: RepeatMode,
    stream: StreamMeta | null = null,
  ) {
    if (queue.length === 0) return;
    await api.restoreSession(queue, index, positionMs, shuffle, repeat);
    const snapshot = await api.getPlaybackState();
    // The engine restores paused without a stream; show the last known stream
    // so the detailed quality readout survives a restart.
    applyEvent(state, { type: "state", ...snapshot, stream: stream ?? snapshot.stream });
  }

  function setVolume(v: number, persist = false) {
    if (v > 0) lastVolume = v;
    return api.setVolume(v, persist);
  }

  function toggleMute() {
    return setVolume(muteTarget(state.volume, lastVolume));
  }

  return {
    get state() { return state; },
    get pendingSeek() { return pendingSeek; },
    init,
    playPause: () => api.playPause(),
    retry,
    restore,
    next: () => api.next(),
    prev: () => api.prev(),
    seek,
    setVolume,
    toggleMute,
    setShuffle: (enabled: boolean) => api.setShuffle(enabled),
    setRepeat: (mode: RepeatMode) => api.setRepeat(mode),
    reorder: (from: number, to: number) => api.queueReorder(from, to),
    remove: (index: number) => api.queueRemove(index),
    clear: () => api.queueClear(),
  };
}

export type Player = ReturnType<typeof createPlayer>;
