import type { LaterEntry, LaterKind, PageItem } from "./api";
import { t } from "./i18n.svelte";

/// Page cap for one walk of the Save for Later list (the Rust id walk uses
/// the same bound), so a broken cursor can never loop forever.
export const MAX_LATER_PAGES = 50;

/// The page's type chips in TIDAL's order (Videos are not shown).
export const LATER_CHIPS: readonly { kind: LaterKind; label: string }[] = [
  { kind: "album", get label() { return t("route.albums"); } },
  { kind: "track", get label() { return t("route.tracks"); } },
  { kind: "artist", get label() { return t("route.profiles"); } },
  { kind: "playlist", get label() { return t("route.playlists"); } },
  { kind: "mix", get label() { return t("nav.mixesAndRadio"); } },
];

/// The id Save for Later stores for an entry (playlists by uuid).
export function laterId(entry: LaterEntry): string {
  return entry.kind === "playlist" ? entry.item.uuid : String(entry.item.id);
}

export function laterEntryKey(entry: LaterEntry): string {
  return `${entry.kind}:${laterId(entry)}`;
}

/// The card a saved entry draws as.
export function laterPageItem(entry: LaterEntry): PageItem {
  switch (entry.kind) {
    case "track": {
      const track = entry.item;
      return {
        id: String(track.id),
        title: track.title,
        subtitle: track.artists.map((a) => a.name).join(", "),
        item_type: "TRACK",
        artwork: track.album?.cover ?? null,
        artist_refs: track.artists,
      };
    }
    case "album": {
      const album = entry.item;
      return {
        id: String(album.id),
        title: album.title,
        subtitle: album.artists.map((a) => a.name).join(", "),
        item_type: "ALBUM",
        artwork: album.cover ?? null,
        release_type: album.type ?? null,
        artist_refs: album.artists,
      };
    }
    case "artist": {
      const artist = entry.item;
      return {
        id: String(artist.id),
        title: artist.name,
        subtitle: t("card.artist"),
        item_type: "ARTIST",
        artwork: artist.picture ?? artist.cover_fallback ?? null,
      };
    }
    case "playlist": {
      const playlist = entry.item;
      const tracks = playlist.numberOfTracks;
      return {
        id: playlist.uuid,
        title: playlist.title,
        subtitle: `${tracks} ${tracks === 1 ? "track" : "tracks"}`,
        item_type: "PLAYLIST",
        artwork: playlist.squareImage ?? playlist.image ?? null,
      };
    }
    case "mix": {
      const mix = entry.item;
      return {
        id: mix.id,
        title: mix.title,
        subtitle: mix.subTitle ?? null,
        item_type: "MIX",
        artwork: mix.image ?? null,
      };
    }
  }
}
