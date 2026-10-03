import type { Artist } from "./api";

export type ArtistSort = "name" | "name-desc";

/// Filter favorite artists by a case-insensitive name query, then sort them.
/// Returns a new array; the input is never mutated.
export function visibleArtists(
  artists: readonly Artist[],
  query: string,
  mode: ArtistSort,
): Artist[] {
  const needle = query.trim().toLowerCase();
  const filtered =
    needle === "" ? [...artists] : artists.filter((a) => a.name.toLowerCase().includes(needle));
  const byName = (a: Artist, b: Artist) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  filtered.sort(mode === "name-desc" ? (a, b) => byName(b, a) : byName);
  return filtered;
}
