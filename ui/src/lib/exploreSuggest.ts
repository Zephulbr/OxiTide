import type { Artist, HistoryEntry } from "./api";
import { groupRecentAlbums, resolveGroups, topArtists, type SuggestApi } from "./homeSuggest";

/// Explore's discovery suggestions (spec 2026-10-16 Explore redesign). Home's
/// shelves revisit what you play; these point away from it: artists close to
/// your top artists that your history and favourites have never touched.

export interface NewArtist {
  artist: Artist;
  /// The top artist this one was suggested from ("Like …").
  seed: string;
}

const lower = (value: string) => value.trim().toLowerCase();

/// Primary artist names ranked by plays in the history window (ties keep
/// recency order). No network: older records without refs use the joined
/// artist string whole, which cannot be split safely.
export function topArtistNames(history: HistoryEntry[], cap = 4): string[] {
  const counts = new Map<string, { name: string; plays: number }>();
  for (const entry of history) {
    const name = (entry.artist_refs?.[0]?.name ?? entry.artist).trim();
    if (!name) continue;
    const key = lower(name);
    const existing = counts.get(key);
    if (existing) existing.plays += 1;
    else counts.set(key, { name, plays: 1 });
  }
  return [...counts.values()]
    .sort((a, b) => b.plays - a.plays)
    .slice(0, cap)
    .map((entry) => entry.name);
}

/// Similar artists of the top `seeds` artists, interleaved one per seed per
/// round so no single seed fills the shelf, minus anyone already heard
/// (history ids or names) or favourited (`isKnown`). A failed lookup only
/// drops that seed's list.
export async function buildNewToYou(
  history: HistoryEntry[],
  api: SuggestApi,
  isKnown: (artistId: number) => boolean = () => false,
  { seeds: seedCap = 4, cap = 16 }: { seeds?: number; cap?: number } = {},
): Promise<NewArtist[]> {
  const groups = groupRecentAlbums(history);
  if (groups.length === 0) return [];
  const resolved = await resolveGroups(groups, api.getTrack);
  const seeds = topArtists(resolved, seedCap);
  if (seeds.length === 0) return [];

  const heardIds = new Set<number>(resolved.map((group) => group.artistId));
  const heardNames = new Set<string>();
  for (const entry of history) {
    heardNames.add(lower(entry.artist));
    for (const ref of entry.artist_refs ?? []) {
      heardIds.add(ref.id);
      heardNames.add(lower(ref.name));
    }
  }

  const settled = await Promise.allSettled(seeds.map((seed) => api.getSimilarArtists(seed.id)));
  const lists = settled.map((result) => (result.status === "fulfilled" ? result.value : []));
  const longest = Math.max(0, ...lists.map((list) => list.length));
  const seen = new Set<number>();
  const out: NewArtist[] = [];
  for (let round = 0; round < longest && out.length < cap; round++) {
    lists.forEach((list, i) => {
      const artist = list[round];
      if (!artist || out.length >= cap || seen.has(artist.id)) return;
      seen.add(artist.id);
      if (heardIds.has(artist.id) || heardNames.has(lower(artist.name)) || isKnown(artist.id)) return;
      out.push({ artist, seed: seeds[i].name });
    });
  }
  return out;
}
