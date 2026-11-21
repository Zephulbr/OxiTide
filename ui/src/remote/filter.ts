import { formsMatch, searchForms, searchNeedle } from "../lib/searchText";
import type { RowTrack, Tile } from "./convert";

/// Lists shorter than this show no filter box: there is nothing to look for.
export const FILTER_MIN = 20;

/// Folded forms per row or tile (`lib/searchText.ts`): rows are stable while a
/// list is shown, so a keystroke reuses them instead of folding every row
/// again.
const formsCache = new WeakMap<object, string[]>();

function formsOf(item: object, fields: () => string[]): string[] {
  let forms = formsCache.get(item);
  if (forms === undefined) {
    forms = fields().flatMap(searchForms);
    formsCache.set(item, forms);
  }
  return forms;
}

/// The in-list filter (Playlist, Tracks, Mix, Album, history): title or
/// artist contains the query, with case, accents and punctuation folded ("ac
/// dc" finds AC/DC), like the desktop's track filter.
export function filterRows(rows: RowTrack[], query: string): RowTrack[] {
  const needle = searchNeedle(query);
  if (needle === "") return rows;
  return rows.filter((row) => formsMatch(formsOf(row, () => [row.title, row.artist]), needle));
}

/// The Library grids' filter: the tile title, plus its subtitle where that is
/// content (an album's artists and year, a mix's description), not a kind
/// label like an artist tile's "Artist".
export function filterTiles(tiles: Tile[], query: string, { subtitle = true } = {}): Tile[] {
  const needle = searchNeedle(query);
  if (needle === "") return tiles;
  return tiles.filter(
    (tile) =>
      formsMatch(searchForms(tile.title), needle) || (subtitle && formsMatch(searchForms(tile.subtitle), needle)),
  );
}

/// A name filter for any list (playlists and folders).
export function filterByName<T>(items: T[], query: string, name: (item: T) => string): T[] {
  const needle = searchNeedle(query);
  if (needle === "") return items;
  return items.filter((item) => formsMatch(searchForms(name(item)), needle));
}

/// The queue for a play from a filtered list: the matches, then the rest of
/// the list, so the collection carries on after the matches (the desktop's
/// `matchesThenRest`). Unfiltered, the matches are the list.
export function matchesThenRest(matches: RowTrack[], all: RowTrack[]): RowTrack[] {
  if (matches === all) return all;
  const matched = new Set(matches);
  return [...matches, ...all.filter((row) => !matched.has(row))];
}
