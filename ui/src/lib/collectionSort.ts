import type { CollectionEntry } from "./api";
import { parseTidalTime } from "./playlistSort";

export type CollectionSort = "date" | "name";

type PlaylistEntry = Extract<CollectionEntry, { kind: "playlist" }>;

/// "date" sorts playlists by their most recent addition (falling back to
/// creation/update), newest first, with folders and playlist slots kept in
/// place. "name" groups folders first, then playlists, each alphabetically
/// and case-insensitively.
export function sortEntries(
  entries: readonly CollectionEntry[],
  mode: CollectionSort,
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
  const sorted = [...playlists].sort((a, b) => recency(b) - recency(a));
  let i = 0;
  return entries.map((entry) => (entry.kind === "playlist" ? sorted[i++] : entry));
}
