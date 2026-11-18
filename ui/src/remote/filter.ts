import type { RowTrack, Tile } from "./convert";

/// Lists shorter than this show no filter box: there is nothing to look for.
export const FILTER_MIN = 20;

function needleOf(query: string): string {
  return query.trim().toLowerCase();
}

/// The in-list filter (Playlist, Tracks, Mix, Album, history): title or
/// artist contains the query, case-insensitive, like the desktop's track
/// filter.
export function filterRows(rows: RowTrack[], query: string): RowTrack[] {
  const needle = needleOf(query);
  if (needle === "") return rows;
  return rows.filter(
    (row) => row.title.toLowerCase().includes(needle) || row.artist.toLowerCase().includes(needle),
  );
}

/// The Library grids' filter: the tile title, plus its subtitle where that is
/// content (an album's artists and year, a mix's description), not a kind
/// label like an artist tile's "Artist".
export function filterTiles(tiles: Tile[], query: string, { subtitle = true } = {}): Tile[] {
  const needle = needleOf(query);
  if (needle === "") return tiles;
  return tiles.filter(
    (tile) =>
      tile.title.toLowerCase().includes(needle) || (subtitle && tile.subtitle.toLowerCase().includes(needle)),
  );
}

/// A name filter for any list (playlists and folders).
export function filterByName<T>(items: T[], query: string, name: (item: T) => string): T[] {
  const needle = needleOf(query);
  if (needle === "") return items;
  return items.filter((item) => name(item).toLowerCase().includes(needle));
}

/// The queue for a play from a filtered list: the matches, then the rest of
/// the list, so the collection carries on after the matches (the desktop's
/// `matchesThenRest`). Unfiltered, the matches are the list.
export function matchesThenRest(matches: RowTrack[], all: RowTrack[]): RowTrack[] {
  if (matches === all) return all;
  const matched = new Set(matches);
  return [...matches, ...all.filter((row) => !matched.has(row))];
}
