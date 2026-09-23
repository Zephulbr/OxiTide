import { listen } from "@tauri-apps/api/event";
import { api, type EngineEvent, type PlaybackState, type Quality, type QueueItem, type RepeatMode, type StreamMeta } from "./api";

export interface PlayerState {
  playing: boolean;
  buffering: boolean;
  current: QueueItem | null;
  position_ms: number;
  duration_ms: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  preferred_quality: Quality;
  output_mode: "shared" | "exclusive";
  bit_perfect: boolean;
  bit_perfect_notice: string | null;
  queue: QueueItem[];
  stream: StreamMeta | null;
  error: string;
}

export function emptyState(): PlayerState {
  return {
    playing: false, buffering: false, current: null, position_ms: 0,
    duration_ms: 0, volume: 1, shuffle: false, repeat: "off",
    preferred_quality: "lossless", queue: [], stream: null, error: "",
    output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
  };
}

export function reduce(state: PlayerState, event: EngineEvent): PlayerState {
  switch (event.type) {
    case "state": {
      const { type, ...rest } = event;
      return { ...state, ...rest };
    }
    case "position":
      return { ...state, position_ms: event.position_ms, duration_ms: event.duration_ms };
    case "now_playing":
      return { ...state, error: "" };
    case "error":
      return { ...state, error: event.message };
    default:
      return state;
  }
}

/** Keeps the slider pinned to the requested position until the engine catches up. */
export function reconcileSeek(pending: number | null, event: EngineEvent): number | null {
  if (pending === null) return null;
  if (event.type === "position" || event.type === "state") {
    return Math.abs(event.position_ms - pending) <= 500 ? null : pending;
  }
  if (event.type === "error" || event.type === "now_playing") return null;
  return pending;
}

export function createPlayer() {
  let state = $state<PlayerState>(emptyState());
  let pendingSeek = $state<number | null>(null);

  async function init() {
    await listen<EngineEvent>("playback_event", (event) => {
      state = reduce(state, event.payload);
      pendingSeek = reconcileSeek(pendingSeek, event.payload);
    });
    try {
      const snapshot = await api.getPlaybackState();
      state = reduce(state, { type: "state", ...snapshot });
    } catch {
      /* not logged in yet */
    }
  }

  function seek(ms: number) {
    pendingSeek = ms;
    return api.seek(ms);
  }

  async function retry() {
    await api.retry();
    // The engine emits a State event, which does not clear the mirrored
    // error; clear it here once the retry command was accepted.
    state = { ...state, error: "" };
  }

  return {
    get state() { return state; },
    get pendingSeek() { return pendingSeek; },
    init,
    playPause: () => api.playPause(),
    retry,
    next: () => api.next(),
    prev: () => api.prev(),
    seek,
    setVolume: (v: number, persist = false) => api.setVolume(v, persist),
    setShuffle: (enabled: boolean) => api.setShuffle(enabled),
    setRepeat: (mode: RepeatMode) => api.setRepeat(mode),
    reorder: (from: number, to: number) => api.queueReorder(from, to),
    remove: (index: number) => api.queueRemove(index),
    clear: () => api.queueClear(),
  };
}

export type Player = ReturnType<typeof createPlayer>;
