import { SvelteSet } from "svelte/reactivity";
import { api } from "./api";
import { invalidatePager, removePagerItem } from "./pager.svelte";

export interface Favorites {
  readonly hydrated: boolean;
  readonly error: string | null;
  readonly trackRevision: number;
  hydrate(): Promise<void>;
  hasTrack(id: number): boolean;
  hasAlbum(id: number): boolean;
  hasArtist(id: number): boolean;
  /// Ids a mounted favorites list should still drop: unfavorited this session,
  /// not re-favorited since, and currently loaded.
  pendingRemovalsToDrop(loaded: ReadonlySet<number>): number[];
  toggleTrack(id: number): Promise<boolean | null>;
  toggleAlbum(id: number): Promise<boolean | null>;
  toggleArtist(id: number): Promise<boolean | null>;
  dismissError(): void;
  reset(): void;
}

type Kind = "track" | "album" | "artist";

/// Favorite state for the signed-in account. Hydrated once per session from
/// `get_favorite_ids`; toggles are optimistic and revert on failure.
export function createFavorites(): Favorites {
  const tracks = new SvelteSet<number>();
  const albums = new SvelteSet<number>();
  const artists = new SvelteSet<number>();
  const pending = new Set<string>();
  /// Ids the user toggled this session; a hydration snapshot taken before the
  /// toggle must not override them.
  const touched = new Set<string>();
  /// Track ids unfavorited this session that a mounted list may still hold.
  const pendingRemovals = new Set<number>();
  let hydrated = $state(false);
  let error = $state<string | null>(null);
  let trackRevision = $state(0);
  let hydratePromise: Promise<void> | null = null;
  let generation = 0;

  function setOf(kind: Kind): SvelteSet<number> {
    if (kind === "album") return albums;
    if (kind === "artist") return artists;
    return tracks;
  }

  function callToggle(kind: Kind, id: number, favorite: boolean): Promise<void> {
    if (kind === "album") return api.setFavoriteAlbum(id, favorite);
    if (kind === "artist") return api.setFavoriteArtist(id, favorite);
    return api.setFavoriteTrack(id, favorite);
  }

  async function hydrate(): Promise<void> {
    if (hydrated) return;
    if (hydratePromise) return hydratePromise;
    const gen = generation;
    hydratePromise = (async () => {
      try {
        const ids = await api.getFavoriteIds();
        if (gen !== generation) return;
        for (const id of ids.tracks) {
          if (!touched.has(`track:${id}`)) tracks.add(id);
        }
        for (const id of ids.albums) {
          if (!touched.has(`album:${id}`)) albums.add(id);
        }
        for (const id of ids.artists) {
          if (!touched.has(`artist:${id}`)) artists.add(id);
        }
        hydrated = true;
      } catch (e) {
        if (gen !== generation) return;
        error = String(e);
      } finally {
        if (gen === generation) hydratePromise = null;
      }
    })();
    return hydratePromise;
  }

  async function toggle(kind: Kind, id: number): Promise<boolean | null> {
    const set = setOf(kind);
    const key = `${kind}:${id}`;
    if (pending.has(key)) return set.has(id);
    const was = set.has(id);
    const next = !was;
    if (next) set.add(id);
    else set.delete(id);
    pending.add(key);
    try {
      await callToggle(kind, id, next);
      touched.add(key);
      error = null;
      if (kind === "track") {
        if (next) {
          invalidatePager("favorites");
        } else {
          removePagerItem("favorites", id);
          pendingRemovals.add(id);
          trackRevision += 1;
        }
      }
      return next;
    } catch (e) {
      if (was) set.add(id);
      else set.delete(id);
      error = String(e);
      return null;
    } finally {
      pending.delete(key);
    }
  }

  return {
    get hydrated() { return hydrated; },
    get error() { return error; },
    get trackRevision() { return trackRevision; },
    hydrate,
    hasTrack: (id) => tracks.has(id),
    hasAlbum: (id) => albums.has(id),
    hasArtist: (id) => artists.has(id),
    pendingRemovalsToDrop: (loaded) => {
      const out: number[] = [];
      for (const id of pendingRemovals) {
        if (!tracks.has(id) && loaded.has(id)) out.push(id);
      }
      return out;
    },
    toggleTrack: (id) => toggle("track", id),
    toggleAlbum: (id) => toggle("album", id),
    toggleArtist: (id) => toggle("artist", id),
    dismissError: () => {
      error = null;
    },
    reset: () => {
      generation += 1;
      hydratePromise = null;
      tracks.clear();
      albums.clear();
      artists.clear();
      pending.clear();
      touched.clear();
      pendingRemovals.clear();
      hydrated = false;
      error = null;
      trackRevision = 0;
    },
  };
}
