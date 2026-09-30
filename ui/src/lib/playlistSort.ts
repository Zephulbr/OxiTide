import type { Playlist } from "./api";

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
