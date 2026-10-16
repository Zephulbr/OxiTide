import type { Album, Artist, PageItem, PageSection } from "./api";

/// Card mappers shared by the artist page and the artist catalog, so both
/// render identical cards.

export function albumCard(album: Album): PageItem {
  const year = album.releaseDate?.slice(0, 4) ?? "";
  return {
    id: String(album.id),
    title: album.title,
    subtitle: /^\d{4}$/.test(year) ? year : null,
    item_type: "ALBUM",
    artwork: album.cover,
    release_type: album.type ?? null,
  };
}

export function artistCard(artist: Artist): PageItem {
  return {
    id: String(artist.id),
    title: artist.name,
    subtitle: null,
    item_type: "ARTIST",
    artwork: artist.picture,
  };
}

export function isPlaylistPageItem(item: PageItem): boolean {
  const type = item.item_type.toUpperCase();
  return type.includes("PLAYLIST") || type.includes("EDITORIAL") || type === "USER";
}

/// The artist feed's playlists module. TIDAL exposes no direct
/// artist-playlists endpoint (spike 2026-10-03), and `pages/artist` returns
/// other mixed modules too, so pick by composition rather than title.
export function playlistsFromSections(sections: PageSection[]): PageItem[] {
  for (const section of sections) {
    if (section.items.length > 0 && section.items.every(isPlaylistPageItem)) {
      return section.items;
    }
  }
  return [];
}
