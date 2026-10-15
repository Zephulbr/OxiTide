import type { Artist, HistoryEntry, Track } from "./api";

/// Home's history-driven shelves. History records carry names only, so one
/// representative track per distinct album is resolved to real album/artist ids
/// (cached for the session), and everything else is derived from those.

export interface RecentGroup {
  key: string;
  album: string;
  artist: string;
  cover: string | null;
  /// Plays of this album within the window.
  plays: number;
  /// Newest play of the album, used to resolve its ids.
  sample: HistoryEntry;
}

export interface ResolvedGroup extends RecentGroup {
  albumId: number;
  artistId: number;
  artistName: string;
}

export interface RadioSeed {
  key: string;
  kind: "artist" | "track";
  id: number;
  title: string;
  subtitle: string;
  cover: string | null;
}

export interface HomeSuggestions {
  albums: ResolvedGroup[];
  radios: RadioSeed[];
  /// The artist the similar-artists shelf is seeded from, with the results.
  similarTo: { id: number; name: string } | null;
  similar: Artist[];
}

export interface SuggestApi {
  getTrack(id: number): Promise<Track>;
  getSimilarArtists(artistId: number): Promise<Artist[]>;
}

const RESOLVE_CAP = 16;

/// Distinct albums in history order (newest first), counting plays per album.
export function groupRecentAlbums(entries: HistoryEntry[], cap = RESOLVE_CAP): RecentGroup[] {
  const groups = new Map<string, RecentGroup>();
  for (const entry of entries) {
    if (!entry.album) continue;
    const key = `${entry.album}\u0000${entry.artist}`;
    const existing = groups.get(key);
    if (existing) {
      existing.plays += 1;
      continue;
    }
    groups.set(key, {
      key,
      album: entry.album,
      artist: entry.artist,
      cover: entry.cover ?? null,
      plays: 1,
      sample: entry,
    });
  }
  return [...groups.values()].slice(0, cap);
}

/// Primary artists ranked by recent plays (ties keep recency order).
export function topArtists(
  groups: ResolvedGroup[],
  cap = 3,
): { id: number; name: string; cover: string | null; plays: number }[] {
  const byId = new Map<number, { id: number; name: string; cover: string | null; plays: number }>();
  for (const group of groups) {
    const existing = byId.get(group.artistId);
    if (existing) existing.plays += group.plays;
    else byId.set(group.artistId, { id: group.artistId, name: group.artistName, cover: group.cover, plays: group.plays });
  }
  return [...byId.values()].sort((a, b) => b.plays - a.plays).slice(0, cap);
}

const trackCache = new Map<number, Promise<Track>>();

/// Test hook: forget resolved tracks.
export function clearSuggestCache() {
  trackCache.clear();
}

function resolveTrack(id: number, getTrack: SuggestApi["getTrack"]): Promise<Track> {
  let hit = trackCache.get(id);
  if (!hit) {
    hit = getTrack(id);
    trackCache.set(id, hit);
    // A failed lookup must not stick for the whole session.
    hit.catch(() => trackCache.delete(id));
  }
  return hit;
}

/// Groups whose representative track resolved to an album and an artist.
export async function resolveGroups(
  groups: RecentGroup[],
  getTrack: SuggestApi["getTrack"],
): Promise<ResolvedGroup[]> {
  const settled = await Promise.allSettled(
    groups.map((group) => resolveTrack(group.sample.track_id, getTrack)),
  );
  const out: ResolvedGroup[] = [];
  settled.forEach((result, i) => {
    if (result.status !== "fulfilled") return;
    const { album, artists } = result.value;
    if (!album || artists.length === 0) return;
    out.push({
      ...groups[i],
      albumId: album.id,
      cover: album.cover ?? groups[i].cover,
      artistId: artists[0].id,
      artistName: artists[0].name,
    });
  });
  return out;
}

/// Build every history-driven shelf from the recent history window.
export async function buildSuggestions(
  history: HistoryEntry[],
  api: SuggestApi,
): Promise<HomeSuggestions> {
  const empty: HomeSuggestions = { albums: [], radios: [], similarTo: null, similar: [] };
  const groups = groupRecentAlbums(history);
  if (groups.length === 0) return empty;
  const albums = await resolveGroups(groups, api.getTrack);
  if (albums.length === 0) return empty;

  const artists = topArtists(albums);
  const radios: RadioSeed[] = artists.map((artist) => ({
    key: `artist-${artist.id}`,
    kind: "artist",
    id: artist.id,
    title: `${artist.name} Radio`,
    subtitle: "Artist radio",
    cover: artist.cover,
  }));
  const latest = history[0];
  if (latest) {
    radios.push({
      key: `track-${latest.track_id}`,
      kind: "track",
      id: latest.track_id,
      title: `${latest.title} Radio`,
      subtitle: "Track radio",
      cover: latest.cover ?? null,
    });
  }

  const seed = artists[0] ?? null;
  let similar: Artist[] = [];
  if (seed) {
    try {
      const known = new Set(artists.map((artist) => artist.id));
      similar = (await api.getSimilarArtists(seed.id)).filter((artist) => !known.has(artist.id));
    } catch {
      similar = [];
    }
  }
  return {
    albums,
    radios,
    similarTo: seed && similar.length > 0 ? { id: seed.id, name: seed.name } : null,
    similar,
  };
}
