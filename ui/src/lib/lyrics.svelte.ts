import { api, type LyricsDto, type QueueItem } from "./api";

export interface LyricsStore {
  readonly loading: boolean;
  readonly lyrics: LyricsDto | null;
  readonly tab: "lyrics" | "next";
  /// The track the current lyrics/loading state describes; it is set as soon
  /// as a fetch starts and kept after a settled miss, so the view can tell a
  /// pending fetch (or a track change the store has not seen yet) apart from
  /// a confirmed "no lyrics".
  readonly trackId: number | null;
  open(track: QueueItem | null): Promise<void>;
  setTab(tab: "lyrics" | "next"): void;
}

export function createLyrics(): LyricsStore {
  let loading = $state(false);
  let lyrics = $state<LyricsDto | null>(null);
  let tab = $state<"lyrics" | "next">("lyrics");
  let trackId = $state<number | null>(null);
  let currentId: number | null = null;

  async function open(track: QueueItem | null) {
    if (!track) {
      currentId = null;
      trackId = null;
      lyrics = null;
      loading = false;
      tab = "lyrics";
      return;
    }
    if (track.track_id === currentId) return;
    currentId = track.track_id;
    trackId = track.track_id;
    // The chosen tab persists across tracks; it only resets when playback
    // stops (the null branch above).
    lyrics = null;
    loading = true;
    let result: LyricsDto | null = null;
    try {
      result = await api.getLyrics(
        track.track_id,
        track.title,
        track.artist,
        track.album,
        track.duration_secs,
      );
    } catch {
      result = null;
    }
    if (currentId !== track.track_id) return; // a newer track superseded this fetch
    if (result === null) {
      // Transient failures and confirmed misses both read as "no lyrics";
      // clear the guard so reopening Now Playing retries. The Rust cache
      // answers confirmed misses from memory without HTTP.
      currentId = null;
    }
    lyrics = result;
    loading = false;
  }

  return {
    get loading() { return loading; },
    get lyrics() { return lyrics; },
    get tab() { return tab; },
    get trackId() { return trackId; },
    open,
    setTab: (next) => { tab = next; },
  };
}

export const lyrics = createLyrics();
