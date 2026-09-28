export type PlaylistRowState = "source" | "active" | "idle";

/// Sidebar row treatment precedence: the playback source wins; otherwise the
/// open (nav-active) playlist keeps its accent treatment; otherwise plain.
export function playlistRowState(isSource: boolean, isActive: boolean): PlaylistRowState {
  if (isSource) return "source";
  if (isActive) return "active";
  return "idle";
}

/// Which playlist started the current queue. Set only by the playlist view's
/// play paths; every other play origin clears it (album, search, collection
/// tracks, mixes) along with the queue's Clear action. Queue jumps and
/// transport controls leave it alone — they keep the same queue.
export function createPlaySource() {
  let playlistId = $state<string | null>(null);
  return {
    get playlistId() { return playlistId; },
    set(id: string | null) { playlistId = id; },
  };
}

export const playSource = createPlaySource();
