import { api, type ArtistDiscography } from "./api";
import type { Route } from "./nav.svelte";
import { prefetch } from "./viewCache";

/// Page data sources, shared by the pages that read them and the hover
/// prefetch, so both hit the same cache key (lib/viewCache.ts).

export const albumSource = (id: number) => ({
  key: `album:${id}`,
  fetch: () => Promise.all([api.getAlbum(id), api.getAlbumTracks(id)]),
});

export const artistSource = (id: number) => ({
  key: `artist:${id}`,
  fetch: () => Promise.all([api.getArtist(id), api.getArtistProfile(id).catch(() => null)]),
});

export const artistTopTracksSource = (id: number) => ({
  key: `artist:${id}:top`,
  fetch: () => api.getArtistTopTracks(id, 20),
});

export const artistAlbumsSource = (id: number, filter: ArtistDiscography) => ({
  key: `artist:${id}:${filter}`,
  fetch: () => api.getArtistAlbums(id, filter, 50, 0),
});

export const similarArtistsSource = (id: number) => ({
  key: `artist:${id}:similar`,
  fetch: () => api.getSimilarArtists(id),
});

export const mixSource = (id: string) => ({
  key: `mix:${id}`,
  fetch: () => api.getMix(id),
});

/// Playlists are edited (here, in the sidebar, on a phone), so the header is
/// always revalidated in the background; the cache only removes the wait.
export const playlistSource = (id: string) => ({
  key: `playlist:${id}`,
  fetch: () => api.getPlaylist(id),
  freshMs: 0,
});

// Library and personal lists change as you use the app (favourites, plays,
// edits on a phone), so they always revalidate in the background.

/// Shared by Home's "New from artists you follow" shelf and the Feed page.
export const feedSource = () => ({ key: "feed", fetch: () => api.getFeed(), freshMs: 0 });

/// Shared by Home's "Made for you" shelf and the Mixes page.
export const forYouSource = () => ({ key: "for-you", fetch: () => api.getForYou() });

export const favoriteAlbumsSource = () => ({
  key: "favorites:albums",
  fetch: () => api.getFavoriteAlbums(),
  freshMs: 0,
});

export const favoriteArtistsSource = () => ({
  key: "favorites:artists",
  fetch: () => api.getFavoriteArtists(),
  freshMs: 0,
});

export const favoriteMixesSource = () => ({
  key: "favorites:mixes",
  fetch: () => api.getFavoriteMixes(),
  freshMs: 0,
});

export const collectionSource = (folderId: string, order: "NAME" | undefined) => ({
  key: `collection:${folderId}:${order ?? "date"}`,
  fetch: () => api.getCollectionItems(folderId, order),
  freshMs: 0,
});

/// Warm the page a card or link is about to open. Only the above-the-fold data:
/// an artist's hero and top tracks (its other shelves load on the visit).
export function prefetchRoute(route: Route | null | undefined) {
  if (!route) return;
  switch (route.name) {
    case "album":
      prefetch(albumSource(route.id));
      break;
    case "artist":
      prefetch(artistSource(route.id));
      prefetch(artistTopTracksSource(route.id));
      break;
    case "mix":
      prefetch(mixSource(route.id));
      break;
    case "playlist":
      prefetch(playlistSource(route.id));
      break;
    case "feed":
      prefetch(feedSource());
      break;
    case "albums":
      prefetch(favoriteAlbumsSource());
      break;
    case "profiles":
      prefetch(favoriteArtistsSource());
      break;
    case "mixes":
      prefetch(forYouSource());
      prefetch(favoriteMixesSource());
      break;
    case "playlists":
      prefetch(collectionSource("root", undefined));
      break;
    case "folder":
      prefetch(collectionSource(route.id, undefined));
      break;
  }
}

/// Hover dwell before a prefetch: a sweep across a carousel or down the
/// sidebar must not fire a request per item it passes.
export const PREFETCH_DWELL_MS = 90;

/// `use:prefetchIntent={route}`: prefetch the route's page once the pointer
/// rests on (or keyboard focus reaches) the element.
export function prefetchIntent(node: HTMLElement, initial: Route) {
  let route = initial;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const start = () => {
    clearTimeout(timer);
    timer = setTimeout(() => prefetchRoute(route), PREFETCH_DWELL_MS);
  };
  const stop = () => clearTimeout(timer);
  node.addEventListener("pointerenter", start);
  node.addEventListener("pointerleave", stop);
  node.addEventListener("focus", start);
  node.addEventListener("blur", stop);
  return {
    update(next: Route) {
      route = next;
    },
    destroy() {
      stop();
      node.removeEventListener("pointerenter", start);
      node.removeEventListener("pointerleave", stop);
      node.removeEventListener("focus", start);
      node.removeEventListener("blur", stop);
    },
  };
}
