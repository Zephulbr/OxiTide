import { SvelteMap } from "svelte/reactivity";
import { api, type Track } from "./api";
import type { SortContext, SortKey } from "./trackSort";

/// Values the `plays` and `released` sort keys need beyond the Track DTO
/// (multi-sort spec §4). Reactive, app-wide, session-only: play counts come
/// from local history, release dates from the album-date store (disk-cached in
/// Rust). Playlist tracks embed their album's date, so `releaseDateOf` reads
/// that first and never asks for it here.
export interface SortData {
  readonly context: SortContext;
  /// Fetch what `keys` need for `tracks`. `fresh` re-reads play counts and
  /// retries albums whose lookup failed earlier (a sort being engaged).
  ensure(keys: readonly SortKey[], tracks: readonly Track[], fresh?: boolean): Promise<void>;
}

type DataApi = Pick<typeof api, "getPlayCounts" | "getAlbumReleaseDates">;

export function createSortData(source: DataApi = api): SortData {
  const plays = new SvelteMap<number, number>();
  const releaseDates = new SvelteMap<number, string | null>();
  /// Albums asked for this session without an answer (failed or offline);
  /// visible-row ensures skip them, a fresh ensure retries them.
  const attempted = new Set<number>();
  const inFlight = new Set<number>();
  const context: SortContext = { plays, releaseDates };

  async function ensurePlays(tracks: readonly Track[], fresh: boolean) {
    const ids = [...new Set(tracks.map((track) => track.id))].filter(
      (id) => fresh || !plays.has(id),
    );
    if (ids.length === 0) return;
    try {
      for (const row of await source.getPlayCounts(ids)) plays.set(row.track_id, row.plays);
    } catch {
      // History unavailable: counts stay unknown and sort last.
    }
  }

  async function ensureReleaseDates(tracks: readonly Track[], fresh: boolean) {
    const wanted = new Set<number>();
    for (const track of tracks) {
      const album = track.album;
      if (!album || album.releaseDate) continue;
      if (releaseDates.has(album.id) || inFlight.has(album.id)) continue;
      if (!fresh && attempted.has(album.id)) continue;
      wanted.add(album.id);
    }
    if (wanted.size === 0) return;
    const ids = [...wanted];
    for (const id of ids) {
      inFlight.add(id);
      attempted.add(id);
    }
    try {
      for (const row of await source.getAlbumReleaseDates(ids)) {
        releaseDates.set(row.album_id, row.release_date);
      }
    } catch {
      // Fail open: the dates stay unknown and those rows sort last.
    } finally {
      for (const id of ids) inFlight.delete(id);
    }
  }

  return {
    context,
    async ensure(keys, tracks, fresh = false) {
      const work: Promise<void>[] = [];
      if (keys.includes("plays")) work.push(ensurePlays(tracks, fresh));
      if (keys.includes("released")) work.push(ensureReleaseDates(tracks, fresh));
      await Promise.all(work);
    },
  };
}

export const sortData: SortData = createSortData();
