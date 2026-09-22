import { listen } from "@tauri-apps/api/event";
import { api, type EngineEvent, type PlaybackState, type QueueItem, type RepeatMode } from "./api";

export interface PlayerState {
  playing: boolean;
  buffering: boolean;
  current: QueueItem | null;
  position_ms: number;
  duration_ms: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  preferred_quality: "low" | "high" | "lossless" | "max";
  output_mode: "shared" | "exclusive";
  bit_perfect: boolean;
  bit_perfect_notice: string | null;
  queue: QueueItem[];
  error: string;
}

export function emptyState(): PlayerState {
  return {
    playing: false, buffering: false, current: null, position_ms: 0,
    duration_ms: 0, volume: 1, shuffle: false, repeat: "off",
    preferred_quality: "lossless", queue: [], error: "",
    output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
  };
}

export function reduce(state: PlayerState, event: EngineEvent): PlayerState {
  switch (event.type) {
    case "state": {
      const { type, ...rest } = event;
      return { ...state, ...rest, error: "" };
    }
    case "position":
      return { ...state, position_ms: event.position_ms, duration_ms: event.duration_ms };
    case "now_playing":
      return state;
    case "error":
      return { ...state, error: event.message };
    default:
      return state;
  }
}

export function createPlayer() {
  let state = $state<PlayerState>(emptyState());

  async function init() {
    await listen<EngineEvent>("playback_event", (event) => {
      state = reduce(state, event.payload);
    });
    try {
      const snapshot = await api.getPlaybackState();
      state = reduce(state, { type: "state", ...snapshot });
    } catch {
      /* not logged in yet */
    }
  }

  return {
    get state() { return state; },
    init,
    playPause: () => api.playPause(),
    next: () => api.next(),
    prev: () => api.prev(),
    seek: (ms: number) => api.seek(ms),
    setVolume: (v: number) => api.setVolume(v),
    setShuffle: (enabled: boolean) => api.setShuffle(enabled),
    setRepeat: (mode: RepeatMode) => api.setRepeat(mode),
    reorder: (from: number, to: number) => api.queueReorder(from, to),
    remove: (index: number) => api.queueRemove(index),
    clear: () => api.queueClear(),
  };
}

export type Player = ReturnType<typeof createPlayer>;
