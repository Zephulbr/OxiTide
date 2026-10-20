import type { Album, Artist, PageItem, Playlist, SearchKind, SearchResults, TopHit } from "./api";

/// Search results as the shared card's items (MediaCard), so the result rows
/// open, play and offer the same menu as Home's.
export function albumCard(album: Album): PageItem {
  return {
    id: String(album.id),
    title: album.title,
    subtitle: album.artists.map((a) => a.name).join(", "),
    item_type: "ALBUM",
    artwork: album.cover ?? null,
    release_type: album.type ?? null,
    artist_refs: album.artists,
  };
}

export function artistCard(artist: Artist): PageItem {
  return {
    id: String(artist.id),
    title: artist.name,
    subtitle: "Artist",
    item_type: "ARTIST",
    artwork: artist.picture ?? artist.cover_fallback ?? null,
  };
}

export function playlistCard(playlist: Playlist): PageItem {
  return {
    id: playlist.uuid,
    title: playlist.title,
    subtitle: `${playlist.numberOfTracks} ${playlist.numberOfTracks === 1 ? "track" : "tracks"}`,
    item_type: "PLAYLIST",
    artwork: playlist.squareImage ?? playlist.image ?? null,
  };
}

/// The overview's Artists row: TIDAL's order, except that artists with no
/// image at all (bare credit-only profiles, often a name variant of a real
/// one) move to the end.
export function artistRow(artists: Artist[]): Artist[] {
  const imaged = (a: Artist) => Boolean(a.picture ?? a.cover_fallback);
  return [...artists.filter(imaged), ...artists.filter((a) => !imaged(a))];
}

const fold =(text: string) =>
  text
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, "");

/// TIDAL's top hit; without one, an artist whose name is the query (ignoring
/// case, accents and punctuation: "ac dc" finds AC/DC). Otherwise none: a
/// guessed top result is worse than the plain list.
export function pickTopHit(results: SearchResults, query: string): TopHit | null {
  if (results.topHit) return results.topHit;
  const wanted = fold(query);
  if (!wanted) return null;
  const artist = results.artists.items.find((a) => fold(a.name) === wanted);
  return artist ? { kind: "artist", item: artist } : null;
}

export type ShelfKind = Exclude<SearchKind, "tracks">;

/// The rows under the tracks: artists, albums, playlists, except that the
/// top hit's own type leads (a query that hits an album wants albums first).
export function shelfOrder(top: TopHit | null): ShelfKind[] {
  const order: ShelfKind[] = ["artists", "albums", "playlists"];
  if (!top || top.kind === "track") return order;
  const lead = `${top.kind}s` as ShelfKind;
  return [lead, ...order.filter((kind) => kind !== lead)];
}

/// The kinds that have anything to show, in chip order.
export function kindsWithResults(results: SearchResults): SearchKind[] {
  return (["tracks", "albums", "artists", "playlists"] as const).filter(
    (kind) => results[kind].items.length > 0,
  );
}

/// How many TIDAL has for the type: its reported total, at least what loaded.
export function totalOf(results: SearchResults, kind: SearchKind): number {
  const group = results[kind];
  return Math.max(group.totalNumberOfItems ?? 0, group.items.length);
}
