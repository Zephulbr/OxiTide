import { vi } from "vitest";
import type { PlaybackState, QueueItem } from "../api";
import type { Player } from "../player.svelte";

export function queueItem(id = 1): QueueItem {
  return {
    track_id: id,
    title: `Track ${id}`,
    artist: "Artist",
    album: "Album",
    duration_secs: 200,
    cover: null,
  };
}

/// A Player stand-in for component tests: real state values, spy methods.
export function fakePlayer(state: Partial<PlaybackState> = {}): Player {
  const base: PlaybackState = {
    playing: true,
    buffering: false,
    current: queueItem(1),
    current_index: 0,
    position_ms: 30000,
    duration_ms: 60000,
    volume: 0.42,
    shuffle: false,
    repeat: "off",
    preferred_quality: "high",
    output_mode: "shared",
    bit_perfect: false,
    bit_perfect_notice: null,
    dsp_active: false,
    dsp_stages: [],
    crossfade_ms: 0,
    queue: [],
    queue_rev: 0,
    stream: null,
  };
  return {
    state: { ...base, error: "", error_code: "", ...state },
    pendingSeek: null,
    seek: vi.fn(),
    setVolume: vi.fn(),
    toggleMute: vi.fn(),
    remove: vi.fn(),
  } as unknown as Player;
}
