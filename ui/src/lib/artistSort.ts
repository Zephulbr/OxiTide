import type { Artist } from "./api";

export type ArtistSort = "date" | "name" | "name-desc";

/// Filter favorite artists by a case-insensitive name query, then sort them.
/// "date" keeps the API's own order (newest first). Returns a new array; the
/// input is never mutated.
export function visibleArtists(
  artists: readonly Artist[],
  query: string,
  mode: ArtistSort,
): Artist[] {
  const needle = query.trim().toLowerCase();
  const filtered =
    needle === "" ? [...artists] : artists.filter((a) => a.name.toLowerCase().includes(needle));
  if (mode === "date") return filtered;
  const byName = (a: Artist, b: Artist) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  filtered.sort(mode === "name-desc" ? (a, b) => byName(b, a) : byName);
  return filtered;
}

export interface ArtistGroup {
  letter: string;
  artists: Artist[];
}

/// Group an already-sorted artist list into consecutive same-initial runs.
/// Diacritics fold to their base letter (Björk -> B); anything else -> "#".
export function groupArtists(artists: readonly Artist[]): ArtistGroup[] {
  const groups: ArtistGroup[] = [];
  for (const artist of artists) {
    const letter = initialOf(artist.name);
    const last = groups[groups.length - 1];
    if (last && last.letter === letter) last.artists.push(artist);
    else groups.push({ letter, artists: [artist] });
  }
  return groups;
}

function initialOf(name: string): string {
  const folded = name
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .trim();
  const first = folded[0]?.toUpperCase() ?? "#";
  return /[A-Z]/.test(first) ? first : "#";
}
