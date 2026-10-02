import type { CollectionEntry, Playlist } from "./api";

/// TIDAL timestamps look like "2026-09-29T20:19:47.004+0000"; the bare
/// `+0000` offset is not part of the ES date-time format, so normalize it
/// to `+00:00` before parsing. Missing or unparseable values sort as
/// oldest (0) so stable sort keeps the API's order for them.
export function parseTidalTime(value: string | null | undefined): number {
  if (!value) return 0;
  const normalized = value.replace(/([+-]\d{2})(\d{2})$/, "$1:$2");
  const parsed = Date.parse(normalized);
  return Number.isNaN(parsed) ? 0 : parsed;
}

/// Playlists with the most recent track additions first. Falls back to
/// `lastUpdated` when TIDAL reports no `lastItemAddedAt` (never-added
/// playlists). Ties — including playlists without any timestamp — keep the
/// API's order because Array.prototype.sort is stable.
export function sortPlaylistsByRecency(playlists: readonly Playlist[]): Playlist[] {
  const key = (playlist: Playlist) =>
    parseTidalTime(playlist.lastItemAddedAt ?? playlist.lastUpdated);
  return [...playlists].sort((a, b) => key(b) - key(a));
}

export type PlaylistSortKey = "recent" | "created" | "updated" | "alpha";
export type SortDirection = "asc" | "desc";
export type PlaylistFilter = "all" | "owned" | "followed";

export interface CollectionSortOptions {
  sort: PlaylistSortKey;
  dir: SortDirection;
  /// Last-played epoch ms for a playlist uuid; 0 when never played.
  rank: (uuid: string) => number;
}

type PlaylistEntry = { kind: "playlist" } & Playlist;

function isPlaylist(entry: CollectionEntry): entry is PlaylistEntry {
  return entry.kind === "playlist";
}

/// Sidebar order: folders first in their input order, then playlists by the
/// selected key and direction. Missing timestamps parse as 0 (oldest), and
/// ties keep the API's order because Array.prototype.sort is stable.
export function sortCollectionEntries(
  entries: readonly CollectionEntry[],
  { sort, dir, rank }: CollectionSortOptions,
): CollectionEntry[] {
  const folders = entries.filter((entry) => entry.kind === "folder");
  const playlists = entries.filter(isPlaylist);
  playlists.sort((a, b) => {
    let result: number;
    switch (sort) {
      case "recent":
        result = rank(a.uuid) - rank(b.uuid);
        break;
      case "created":
        result = parseTidalTime(a.created) - parseTidalTime(b.created);
        break;
      case "updated":
        result = parseTidalTime(a.lastUpdated) - parseTidalTime(b.lastUpdated);
        break;
      case "alpha":
        result = a.title.localeCompare(b.title, undefined, { sensitivity: "base" });
        break;
    }
    return dir === "asc" ? result : -result;
  });
  return [...folders, ...playlists];
}

/// Sidebar filter: `owned` keeps folders and the user's playlists; `followed`
/// keeps only playlists the user does not own (folders hold owned playlists,
/// so they are hidden there).
export function filterCollectionEntries(
  entries: readonly CollectionEntry[],
  filter: PlaylistFilter,
): CollectionEntry[] {
  if (filter === "all") return [...entries];
  if (filter === "owned") {
    return entries.filter((entry) => entry.kind === "folder" || entry.owned);
  }
  return entries.filter((entry) => isPlaylist(entry) && !entry.owned);
}
