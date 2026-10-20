import type { Album, Artist, Playlist, SearchKind, SearchResults, Track } from "./api";
import { totalOf } from "./searchResults";

interface Items {
  tracks: Track[];
  albums: Album[];
  artists: Artist[];
  playlists: Playlist[];
}

export interface SearchPages {
  items<K extends SearchKind>(kind: K): Items[K];
  total(kind: SearchKind): number;
  hasMore(kind: SearchKind): boolean;
  loading(kind: SearchKind): boolean;
  error(kind: SearchKind): string;
  /// A new overview arrived: every kind restarts from its first page.
  seed(results: SearchResults, query: string): void;
  /// Appends the kind's next page.
  more(kind: SearchKind): Promise<void>;
}

const KINDS: SearchKind[] = ["tracks", "albums", "artists", "playlists"];
const keyOf = (kind: SearchKind, item: Track | Album | Artist | Playlist) =>
  kind === "playlists" ? (item as Playlist).uuid : (item as Track).id;

/// Explore's per-type views: the overview's first page of each type, then
/// further pages on request. A page that lands after the query moved on is
/// dropped (latest wins, as in `createSearch`).
export function createSearchPages(
  fetchPage: (query: string, kind: SearchKind, offset: number) => Promise<SearchResults>,
): SearchPages {
  let query = "";
  let generation = 0;
  const items = $state<Items>({ tracks: [], albums: [], artists: [], playlists: [] });
  const totals = $state<Record<SearchKind, number>>({ tracks: 0, albums: 0, artists: 0, playlists: 0 });
  const loading = $state<Record<SearchKind, boolean>>({ tracks: false, albums: false, artists: false, playlists: false });
  const errors = $state<Record<SearchKind, string>>({ tracks: "", albums: "", artists: "", playlists: "" });

  function seed(results: SearchResults, next: string) {
    query = next;
    generation += 1;
    for (const kind of KINDS) {
      (items[kind] as unknown[]) = [...results[kind].items];
      totals[kind] = totalOf(results, kind);
      loading[kind] = false;
      errors[kind] = "";
    }
  }

  async function more(kind: SearchKind) {
    if (loading[kind] || items[kind].length >= totals[kind]) return;
    const token = generation;
    loading[kind] = true;
    errors[kind] = "";
    try {
      const page = await fetchPage(query, kind, items[kind].length);
      if (token !== generation) return;
      const seen = new Set(items[kind].map((item) => keyOf(kind, item)));
      const fresh = (page[kind].items as (Track | Album | Artist | Playlist)[]).filter(
        (item) => !seen.has(keyOf(kind, item)),
      );
      (items[kind] as unknown[]) = [...items[kind], ...fresh];
      // An empty or all-duplicate page ends the list whatever the total says.
      totals[kind] = fresh.length === 0 ? items[kind].length : Math.max(totals[kind], items[kind].length);
    } catch (e) {
      if (token === generation) errors[kind] = String(e);
    } finally {
      if (token === generation) loading[kind] = false;
    }
  }

  return {
    items: (kind) => items[kind],
    total: (kind) => totals[kind],
    hasMore: (kind) => items[kind].length < totals[kind],
    loading: (kind) => loading[kind],
    error: (kind) => errors[kind],
    seed,
    more,
  };
}
