import type { CollectionEntry } from "./api";

export type CollectionSort = "date" | "name";

/// "date" keeps the API's newest-first order; "name" groups folders first,
/// then playlists, each alphabetically and case-insensitively.
export function sortEntries(
  entries: readonly CollectionEntry[],
  mode: CollectionSort,
): CollectionEntry[] {
  if (mode === "date") return [...entries];
  const nameOf = (entry: CollectionEntry) => (entry.kind === "folder" ? entry.name : entry.title);
  const byName = (a: CollectionEntry, b: CollectionEntry) =>
    nameOf(a).localeCompare(nameOf(b), undefined, { sensitivity: "base" });
  const folders = entries.filter((entry) => entry.kind === "folder").sort(byName);
  const playlists = entries.filter((entry) => entry.kind === "playlist").sort(byName);
  return [...folders, ...playlists];
}
