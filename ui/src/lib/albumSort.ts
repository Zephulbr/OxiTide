import type { Album } from "./api";
import { parseTidalTime } from "./playlistSort";

export type AlbumSort = "added" | "released" | "title" | "artist";

const byText = (a: string, b: string) => a.localeCompare(b, undefined, { sensitivity: "base", numeric: true });
const artistOf = (album: Album) => album.artists[0]?.name ?? "";

/// "added" puts the newest favorites first and "released" the newest
/// releases; albums without the date keep the API's order at the end (the
/// sort is stable). "title" is A–Z. "artist" is A–Z by the first credited
/// artist, each artist's albums newest release first.
export function sortAlbums(albums: readonly Album[], mode: AlbumSort): Album[] {
  const sorted = [...albums];
  switch (mode) {
    case "added":
      return sorted.sort((a, b) => parseTidalTime(b.dateAdded) - parseTidalTime(a.dateAdded));
    case "released":
      return sorted.sort((a, b) => parseTidalTime(b.releaseDate) - parseTidalTime(a.releaseDate));
    case "title":
      return sorted.sort((a, b) => byText(a.title, b.title));
    case "artist":
      return sorted.sort(
        (a, b) =>
          byText(artistOf(a), artistOf(b)) || parseTidalTime(b.releaseDate) - parseTidalTime(a.releaseDate),
      );
  }
}
