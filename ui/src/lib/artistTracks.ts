import { api, type ArtistRef, type Track } from "./api";
import { claimPlay, extendPlay, restAfter, startPlay, type Started } from "./contextPlay";
import { createTrackPager, type Pager } from "./pager.svelte";
import type { PlayFrom } from "./playSource.svelte";

/// The artist page's Tracks tab (every track, most popular first). Shared by
/// that tab and search plays, so a list loaded by one is cached for the other.
export function artistTracksPager(id: number): Pager {
  return createTrackPager(`artist:${id}:tracks`, (offset, limit) =>
    api.getArtistTopTracksPage(id, offset, limit),
  );
}

/// Every track of the artist, loaded in full; throws when a page fails.
export async function loadArtistTracks(id: number): Promise<Track[]> {
  const pager = artistTracksPager(id);
  await pager.ensureAll();
  if (pager.error) throw new Error(pager.error);
  return pager.items;
}

/// `first`, then the artist's other tracks in their order (no repeat of it).
export function firstThenArtist(first: Track | undefined, artistTracks: Track[]): Track[] {
  if (!first) return artistTracks;
  return [first, ...artistTracks.filter((track) => track.id !== first.id)];
}

/// What a search play waits for: one request, so the song starts at once.
export const FIRST_PAGE = 100;

/// Play every track of `artist`, starting with `first` when given (a song
/// picked from search), as TIDAL does (issue #3). With shuffle on, the engine
/// plays `first` and shuffles the rest. Unless the artist's list is already
/// cached, playback starts from its first page and the other tracks join the
/// queue once loaded (`lib/contextPlay.ts`; another play in between cancels
/// that). `false` when nothing is playable (the caller says so); a failed
/// first page throws.
export async function playArtistTracks(artist: ArtistRef, first?: Track): Promise<boolean> {
  const isLatest = claimPlay();
  const from: PlayFrom = { kind: "artist", name: artist.name, route: { name: "artist", id: artist.id } };
  const pager = artistTracksPager(artist.id);
  if (pager.done) {
    return (await startPlay(from, firstThenArtist(first, pager.items), 0, { isLatest })) !== "hidden";
  }

  const page = await api.getArtistTopTracksPage(artist.id, 0, FIRST_PAGE);
  const head = firstThenArtist(first, page.items);
  const started = await startPlay(from, head, 0, { isLatest });
  if (typeof started === "string") return started !== "hidden";
  const more = page.total > 0 ? page.total > FIRST_PAGE : page.items.length === FIRST_PAGE;
  if (more) void extendWithRest(artist.id, from, head, started);
  return true;
}

/// The second half of a search play: the artist's tracks past the first
/// page. Best effort: a failed load just leaves the first page queued.
async function extendWithRest(id: number, from: PlayFrom, head: Track[], started: Started) {
  try {
    await extendPlay(from, started, restAfter(head, await loadArtistTracks(id)));
  } catch {
    // The first page is already playing; the rest is a bonus.
  }
}
