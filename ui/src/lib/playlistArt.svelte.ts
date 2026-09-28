import { SvelteMap } from "svelte/reactivity";
import { api } from "./api";
import { collageCovers } from "./artwork";
import { cachedPagerItems } from "./pager.svelte";

/// Rows fetched per playlist to find the first four distinct albums; the
/// pager session cache is preferred when that playlist was already opened.
export const PLAYLIST_COVERS_LIMIT = 20;
const MAX_CONCURRENT = 4;

/// Session cache of collage tiles keyed by playlist uuid. A missing entry
/// means "not requested yet"; an empty array means "requested, no covers".
const covers = new SvelteMap<string, string[]>();
const epochs = new Map<string, number>();
const pending = new Set<string>();
const queue: string[] = [];
let active = 0;
let generation = 0;

/// Reactive peek at a playlist's collage tiles.
export function playlistCovers(uuid: string): string[] | null {
  return covers.get(uuid) ?? null;
}

/// Ask for a playlist's collage tiles once; repeat calls are no-ops while
/// the fetch is queued, in flight, or already cached.
export function requestPlaylistCovers(uuid: string): void {
  if (!uuid || covers.has(uuid) || pending.has(uuid)) return;
  const cached = cachedPagerItems(`playlist:${uuid}`);
  if (cached) {
    covers.set(uuid, collageCovers(cached.map((t) => t.album?.cover)));
    return;
  }
  pending.add(uuid);
  queue.push(uuid);
  pump();
}

function pump(): void {
  while (active < MAX_CONCURRENT && queue.length > 0) {
    const uuid = queue.shift()!;
    const epoch = epochs.get(uuid) ?? 0;
    const gen = generation;
    active += 1;
    const current = () => gen === generation && (epochs.get(uuid) ?? 0) === epoch;
    api
      .getPlaylistTracksPage(uuid, 0, PLAYLIST_COVERS_LIMIT)
      .then((page) => {
        if (current()) covers.set(uuid, collageCovers(page.items.map((t) => t.album?.cover)));
      })
      .catch(() => {
        // Cache the empty result so a dead playlist is not retried in a loop;
        // invalidation or a restart gives it another chance.
        if (current()) covers.set(uuid, []);
      })
      .finally(() => {
        active -= 1;
        pending.delete(uuid);
        pump();
      });
  }
}

/// Drop one playlist's tiles; an in-flight response for it is discarded.
export function invalidatePlaylistCovers(uuid: string): void {
  covers.delete(uuid);
  epochs.set(uuid, (epochs.get(uuid) ?? 0) + 1);
}

/// Drop everything; used when the signed-in account changes.
export function clearPlaylistCovers(): void {
  covers.clear();
  epochs.clear();
  queue.length = 0;
  pending.clear();
  generation += 1;
}
