import type { RowTrack } from "./convert";

/// The in-list filter (Playlist, Tracks): title or artist contains the
/// query, case-insensitive, like the desktop's track filter.
export function filterRows(rows: RowTrack[], query: string): RowTrack[] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return rows;
  return rows.filter(
    (row) => row.title.toLowerCase().includes(needle) || row.artist.toLowerCase().includes(needle),
  );
}

/// The queue for a play from a filtered list: the matches, then the rest of
/// the list, so the collection carries on after the matches (the desktop's
/// `matchesThenRest`). Unfiltered, the matches are the list.
export function matchesThenRest(matches: RowTrack[], all: RowTrack[]): RowTrack[] {
  if (matches === all) return all;
  const matched = new Set(matches);
  return [...matches, ...all.filter((row) => !matched.has(row))];
}
