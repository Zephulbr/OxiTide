import { api, type LyricsDto, type QueueItem } from "./api";

export interface LyricsStore {
  readonly loading: boolean;
  readonly lyrics: LyricsDto | null;
  readonly tab: "lyrics" | "next";
  open(track: QueueItem | null): Promise<void>;
  setTab(tab: "lyrics" | "next"): void;
}

export function createLyrics(): LyricsStore {
  let loading = $state(false);
  let lyrics = $state<LyricsDto | null>(null);
  let tab = $state<"lyrics" | "next">("lyrics");
  let currentId: number | null = null;

  async function open(track: QueueItem | null) {
    if (!track) {
      currentId = null;
      lyrics = null;
      loading = false;
      tab = "lyrics";
      return;
    }
    if (track.track_id === currentId) return;
    currentId = track.track_id;
    tab = "lyrics";
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
    lyrics = result;
    loading = false;
  }

  return {
    get loading() { return loading; },
    get lyrics() { return lyrics; },
    get tab() { return tab; },
    open,
    setTab: (next) => { tab = next; },
  };
}

export const lyrics = createLyrics();
