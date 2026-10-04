import { api } from "./api";
import { playlistRecency } from "./playlistRecency.svelte";

export type PlaylistRowState = "source" | "active" | "source-active" | "idle";

/// Sidebar row treatment: playing (source) and open (nav-active) are separate
/// signals, so a row can be both. The playing source keeps its accent label
/// and play glyph; the open highlight is never lost to it.
export function playlistRowState(isSource: boolean, isActive: boolean): PlaylistRowState {
  if (isSource) return isActive ? "source-active" : "source";
  if (isActive) return "active";
  return "idle";
}

/// Which playlist started the current queue. Set only by the playlist view's
/// play paths; every other play origin clears it (album, search, collection
/// tracks, mixes) along with the queue's Clear action. Queue jumps and
/// transport controls leave it alone — they keep the same queue. `onPlay`
/// fires whenever a playlist becomes the source (never on clearing), letting
/// the recency store count every listened-to playlist.
export function createPlaySource(
  onPlay?: (uuid: string) => void,
  persist?: (id: string | null) => void,
) {
  let playlistId = $state<string | null>(null);
  return {
    get playlistId() { return playlistId; },
    set(id: string | null) {
      playlistId = id;
      persist?.(id);
      if (id !== null) onPlay?.(id);
    },
    /// Re-apply a saved source without recording a new play (session restore),
    /// and persist it so the saver cannot clobber the marker on its next write.
    restore(id: string | null) {
      playlistId = id;
      persist?.(id);
    },
  };
}

export const playSource = createPlaySource(
  (id) => playlistRecency().record(id),
  (id) => { void api.setPlaySource(id).catch(() => {}); },
);
