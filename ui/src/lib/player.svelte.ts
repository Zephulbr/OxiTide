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

export function createPlayer() {
  let state = $state<PlayerState>(emptyState());
  let pendingSeek = $state<number | null>(null);
  let lastVolume = 0.5;

  async function init() {
    await listen<EngineEvent>("playback_event", (event) => {
      state = reduce(state, event.payload);
      pendingSeek = reconcileSeek(pendingSeek, event.payload);
    });
    try {
      const snapshot = await api.getPlaybackState();
      state = reduce(state, { type: "state", ...snapshot });
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
    // The engine emits a State event, which does not clear the mirrored
    // error; clear it here once the retry command was accepted.
    state = { ...state, error: "" };
  }

  /// Rebuild the queue after a re-login: start the context, seek to the last
  /// position, and pause again when the session was paused. `play_context`
  /// always starts playback, hence the extra `playPause` for paused sessions.
  async function restore(
    queue: QueueItem[],
    index: number,
    positionMs: number,
    wasPlaying: boolean,
  ) {
    if (queue.length === 0) return;
    await api.playTracks(queue, index);
    await seek(positionMs);
    if (!wasPlaying) await api.playPause();
    const snapshot = await api.getPlaybackState();
    state = reduce(state, { type: "state", ...snapshot });
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
