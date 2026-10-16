import type { Mix, PageItem, PageTrackInput, Playlist } from "./api";

export type CardShape = "artist" | "album" | "playlist" | "mix" | "plain";
export type SectionTreatment = "tiles" | "carousel" | "list";

const NUMERIC_ID = /^\d+$/;
const upper = (value: string) => value.toUpperCase();

/// Silhouette for a page item, keyed on TIDAL's item type.
export function shapeForItemType(itemType: string): CardShape {
  const type = upper(itemType);
  if (type.includes("ARTIST")) return "artist";
  if (type.includes("ALBUM")) return "album";
  if (type === "MIX" || type === "MIX_LIST") return "mix";
  if (type.includes("PLAYLIST") || type.includes("EDITORIAL")) return "playlist";
  return "plain";
}

/// Full-length albums (not EPs or singles) get square-cornered covers. An
/// unknown release type is not assumed to be one.
export function isAlbumRelease(releaseType: string | null | undefined): boolean {
  return upper(releaseType ?? "") === "ALBUM";
}

export function isFullAlbum(item: PageItem): boolean {
  return shapeForItemType(item.item_type) === "album" && isAlbumRelease(item.release_type);
}

function isTrackish(type: string): boolean {
  return type.includes("TRACK") || type.includes("VIDEO");
}

/// Section treatment from the TIDAL module kind plus item-type composition.
/// Unknown kinds fall back to carousel so new feed rows never break Home.
export function classifySection(kind: string, items: PageItem[]): SectionTreatment {
  if (items.length === 0) return "carousel";
  const k = upper(kind);
  const types = items.map((item) => upper(item.item_type));
  const trackish = types.filter(isTrackish).length;
  if (trackish * 2 > items.length) return "list";
  const mixish =
    k.includes("MIX") ||
    k.includes("RADIO") ||
    types.some((type) => type.includes("MIX") || type.includes("RADIO"));
  if (mixish) return "carousel";
  if (k.includes("ALBUM")) return "carousel";
  const playlistish = types.filter(
    (type) => type.includes("PLAYLIST") || type.includes("EDITORIAL"),
  ).length;
  if (playlistish * 2 > items.length || k.includes("PLAYLIST") || k.includes("EDITORIAL")) {
    return "tiles";
  }
  return "carousel";
}

/// Track items are only playable when their id can be sent to the backend.
export function isPlayableTrack(item: PageItem): boolean {
  return upper(item.item_type).includes("TRACK") && NUMERIC_ID.test(item.id);
}

/// Numeric track ids in section order, skipping everything else.
export function playableTrackIds(items: PageItem[]): number[] {
  return items.filter(isPlayableTrack).map((item) => Number(item.id));
}

/// Playable page item → the payload for `playPageItems`. Per-field nulls mark
/// metadata the payload did not carry; the backend resolves those ids.
export function pageTrackInput(item: PageItem): PageTrackInput {
  return {
    id: Number(item.id),
    title: item.title || null,
    artists: item.artists && item.artists.length > 0 ? item.artists : null,
    album: item.album_title ?? null,
    duration: item.duration ?? null,
    cover: item.artwork ?? null,
    explicit: item.explicit ?? null,
  };
}

export interface ShortcutEntry {
  key: string;
  item: PageItem;
  playlist: boolean;
  route: { name: "playlist"; id: string } | { name: "mix"; id: string };
}

/// Home's quick-access strip: owned playlists first, then saved mixes.
export function shortcutEntries(
  playlists: Playlist[],
  mixes: Mix[],
  cap = 6,
): ShortcutEntry[] {
  const entries: ShortcutEntry[] = [];
  for (const p of playlists) {
    entries.push({
      key: `playlist-${p.uuid}`,
      playlist: true,
      route: { name: "playlist", id: p.uuid },
      item: {
        id: p.uuid,
        title: p.title,
        subtitle: "Playlist",
        item_type: "PLAYLIST_LIST",
        artwork: p.squareImage ?? p.image ?? null,
      },
    });
  }
  for (const m of mixes) {
    entries.push({
      key: `mix-${m.id}`,
      playlist: false,
      route: { name: "mix", id: m.id },
      item: {
        id: m.id,
        title: m.title,
        subtitle: "Saved mix",
        item_type: "MIX_LIST",
        artwork: m.image ?? null,
      },
    });
  }
  return entries.slice(0, cap);
}
