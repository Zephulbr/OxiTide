import type { CollectionEntry } from "./api";
import { parseTidalTime } from "./playlistSort";

export type CollectionSort = "date" | "name" | "played";

type PlaylistEntry = Extract<CollectionEntry, { kind: "playlist" }>;

/// "date" sorts playlists by their most recent addition (falling back to
/// creation/update), newest first, with folders and playlist slots kept in
/// place. "played" does the same by last-played time (`playedAt`, 0 when
/// never played; ties and never-played fall back to "date"). "name" groups
/// folders first, then playlists, each alphabetically and case-insensitively.
export function sortEntries(
  entries: readonly CollectionEntry[],
  mode: CollectionSort,
  playedAt: (uuid: string) => number = () => 0,
): CollectionEntry[] {
  if (mode === "name") {
    const nameOf = (entry: CollectionEntry) => (entry.kind === "folder" ? entry.name : entry.title);
    const byName = (a: CollectionEntry, b: CollectionEntry) =>
      nameOf(a).localeCompare(nameOf(b), undefined, { sensitivity: "base" });
    const folders = entries.filter((entry) => entry.kind === "folder").sort(byName);
    const playlists = entries.filter((entry) => entry.kind === "playlist").sort(byName);
    return [...folders, ...playlists];
  }
  const playlists = entries.filter((entry): entry is PlaylistEntry => entry.kind === "playlist");
  const recency = (entry: PlaylistEntry) =>
    parseTidalTime(entry.lastItemAddedAt ?? entry.created ?? entry.lastUpdated);
  const sorted = [...playlists].sort(
    (a, b) => (mode === "played" ? playedAt(b.uuid) - playedAt(a.uuid) : 0) || recency(b) - recency(a),
  );
  let i = 0;
  return entries.map((entry) => (entry.kind === "playlist" ? sorted[i++] : entry));
}
