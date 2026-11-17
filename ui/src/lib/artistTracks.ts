import { api, trackToQueueItem, type ArtistRef, type Track } from "./api";
import { createTrackPager, type Pager } from "./pager.svelte";
import { playSource, type PlayFrom } from "./playSource.svelte";
import { trackFlags } from "./trackFlags.svelte";

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
/// queue once loaded (`queueExtendContext`, which a queue edit or another
/// play in between cancels). `false` when nothing is playable (the caller
/// says so); a failed first page throws.
export async function playArtistTracks(artist: ArtistRef, first?: Track): Promise<boolean> {
  const from: PlayFrom = { kind: "artist", name: artist.name, route: { name: "artist", id: artist.id } };
  const pager = artistTracksPager(artist.id);
  if (pager.done) return (await play(from, firstThenArtist(first, pager.items))) !== null;

  const page = await api.getArtistTopTracksPage(artist.id, 0, FIRST_PAGE);
  const head = firstThenArtist(first, page.items);
  const started = await play(from, head);
  if (started === null) return false;
  const more = page.total > 0 ? page.total > FIRST_PAGE : page.items.length === FIRST_PAGE;
  if (more) void extendWithRest(artist.id, from, head, started);
  return true;
}

interface Started {
  rev: number;
  ids: number[];
}

async function play(from: PlayFrom, list: Track[]): Promise<Started | null> {
  if (list.length === 0) return null;
  const prepared = await trackFlags.filterPlayable(list, 0);
  if (!prepared) return null;
  const ids = prepared.items.map((track) => track.id);
  playSource.set(from, ids);
  const rev = await api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  return { rev, ids };
}

/// The second half of a search play: the artist's tracks past the first
/// page. Best effort: a failed load just leaves the first page queued.
async function extendWithRest(id: number, from: PlayFrom, head: Track[], started: Started) {
  try {
    const queued = new Set(head.map((track) => track.id));
    const rest = (await loadArtistTracks(id)).filter((track) => !queued.has(track.id));
    if (rest.length === 0) return;
    const prepared = await trackFlags.filterPlayable(rest, 0);
    if (!prepared) return;
    await api.queueExtendContext(prepared.items.map(trackToQueueItem), started.rev);
    // Credit the added tracks to the artist, unless another play took over
    // (by value: the store keeps its own proxied copy of `from`).
    const now = playSource.from;
    if (now?.kind === from.kind && now.name === from.name && now.route?.name === "artist" && now.route.id === id) {
      playSource.set(from, [...started.ids, ...prepared.items.map((track) => track.id)]);
    }
  } catch {
    // The first page is already playing; the rest is a bonus.
  }
}
