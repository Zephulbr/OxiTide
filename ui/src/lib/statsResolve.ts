import type { Artist, StatsTopEntry, Track } from "./api";
import { resolveTrack } from "./homeSuggest";

/// History's top lists carry names only; each entry's newest play resolves it
/// to real ids (and the artist picture) so the Stats rows can open pages.

export interface StatsResolveApi {
  getTrack(id: number): Promise<Track>;
  getArtist(id: number): Promise<Artist>;
}

export interface ArtistTarget {
  id: number;
  picture: string | null;
}

const artists = new Map<number, Promise<Artist>>();

/// Test hook: forget fetched artists.
export function clearStatsResolveCache() {
  artists.clear();
}

function cachedArtist(id: number, getArtist: StatsResolveApi["getArtist"]): Promise<Artist> {
  let hit = artists.get(id);
  if (!hit) {
    hit = getArtist(id);
    artists.set(id, hit);
    hit.catch(() => artists.delete(id));
  }
  return hit;
}

/// The artist behind a top-artist entry. History joins a track's artists into
/// one string, so the credit whose name matches wins, else the primary one. A
/// failed picture lookup keeps the id.
export async function resolveArtist(
  entry: StatsTopEntry,
  api: StatsResolveApi,
): Promise<ArtistTarget | null> {
  const track = await resolveTrack(entry.track_id, api.getTrack);
  const ref = track.artists.find((artist) => artist.name === entry.name) ?? track.artists[0];
  if (!ref) return null;
  try {
    const artist = await cachedArtist(ref.id, api.getArtist);
    return { id: ref.id, picture: artist.picture ?? null };
  } catch {
    return { id: ref.id, picture: null };
  }
}

/// The album id behind a top-album entry, if its newest play still resolves.
export async function resolveAlbum(entry: StatsTopEntry, api: StatsResolveApi): Promise<number | null> {
  const track = await resolveTrack(entry.track_id, api.getTrack);
  return track.album?.id ?? null;
}
