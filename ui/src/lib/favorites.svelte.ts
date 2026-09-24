import { SvelteSet } from "svelte/reactivity";
import { api } from "./api";
import { invalidatePager, removePagerItem } from "./pager.svelte";

export interface Favorites {
  readonly hydrated: boolean;
  readonly error: string | null;
  readonly trackRevision: number;
  readonly lastUnfavoritedTrackId: number | null;
  hydrate(): Promise<void>;
  hasTrack(id: number): boolean;
  hasAlbum(id: number): boolean;
  hasArtist(id: number): boolean;
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
  let hydrated = $state(false);
  let error = $state<string | null>(null);
  let trackRevision = $state(0);
  let lastUnfavoritedTrackId = $state<number | null>(null);
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
        for (const id of ids.tracks) tracks.add(id);
        for (const id of ids.albums) albums.add(id);
        for (const id of ids.artists) artists.add(id);
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
      error = null;
      if (kind === "track") {
        if (next) {
          invalidatePager("favorites");
        } else {
          removePagerItem("favorites", id);
          lastUnfavoritedTrackId = id;
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
    get lastUnfavoritedTrackId() { return lastUnfavoritedTrackId; },
    hydrate,
    hasTrack: (id) => tracks.has(id),
    hasAlbum: (id) => albums.has(id),
    hasArtist: (id) => artists.has(id),
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
      hydrated = false;
      error = null;
      trackRevision = 0;
      lastUnfavoritedTrackId = null;
    },
  };
}
