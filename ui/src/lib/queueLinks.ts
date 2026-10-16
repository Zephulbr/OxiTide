import type { ArtistRef, Track } from "./api";
import { resolveTrack } from "./homeSuggest";
import { menuArtists } from "./nameLinks";
import type { Route } from "./nav.svelte";

/// Name-link targets for the playing-track surfaces (queue items, history
/// entries). Items carry ids when their source had them (name-links spec
/// NL-D8); older or page-feed items resolve on click through the
/// session-cached `resolveTrack`, never on render.
export interface LinkSource {
  track_id: number;
  artist: string;
  album?: string;
  artist_refs?: ArtistRef[];
  album_id?: number | null;
}

type GetTrack = (id: number) => Promise<Track>;

async function lookup(item: LinkSource, getTrack: GetTrack): Promise<Track | null> {
  try {
    return await resolveTrack(item.track_id, getTrack);
  } catch {
    return null;
  }
}

export async function albumRoute(item: LinkSource, getTrack: GetTrack): Promise<Route | null> {
  if (item.album_id !== undefined && item.album_id !== null) return { name: "album", id: item.album_id };
  const album = (await lookup(item, getTrack))?.album;
  return album ? { name: "album", id: album.id } : null;
}

/// "Go to" menu targets for an item (the keyboard path to its links). Items
/// without ids still get entries; they resolve when chosen.
export function goToTargets(
  item: LinkSource,
  go: (route: Route) => void,
  getTrack: GetTrack,
): { goToAlbum?: () => void; goToArtists: { name: string; open(): void }[] } {
  const follow = (find: Promise<Route | null>) => void find.then((route) => route && go(route));
  const hasAlbum = (item.album_id !== undefined && item.album_id !== null) || Boolean(item.album);
  const refs = menuArtists(item.artist_refs ?? [], null);
  const goToArtists =
    refs.length > 0
      ? refs.map((a) => ({ name: a.name, open: () => go({ name: "artist", id: a.id }) }))
      : item.artist
        ? [{ name: item.artist, open: () => follow(primaryArtistRoute(item, getTrack)) }]
        : [];
  return {
    goToAlbum: hasAlbum ? () => follow(albumRoute(item, getTrack)) : undefined,
    goToArtists,
  };
}

/// The primary artist, for an item whose joined artist string is one link.
export async function primaryArtistRoute(item: LinkSource, getTrack: GetTrack): Promise<Route | null> {
  const known = item.artist_refs?.[0];
  if (known) return { name: "artist", id: known.id };
  const artist = (await lookup(item, getTrack))?.artists[0];
  return artist ? { name: "artist", id: artist.id } : null;
}
