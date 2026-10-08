import type { Route } from "../nav.svelte";

/// The selector unit's lit source keys (RK-D4). Search is momentary and has no
/// lamp of its own.
export type Source = "home" | "explore" | "library" | "playlists" | "settings";

const LIBRARY: Route["name"][] = [
  "albums", "tracks", "profiles", "mixes", "purchases", "history", "feed",
  "album", "artist", "artist-all", "mix",
];

export function sourceOf(route: Route): Source | null {
  switch (route.name) {
    case "home":
      return "home";
    case "explore":
    case "section":
      return "explore";
    case "playlists":
    case "folder":
    case "playlist":
      return "playlists";
    case "settings":
      return "settings";
    default:
      return LIBRARY.includes(route.name) ? "library" : null;
  }
}

/// Sub-source keys shown above the page while browsing the collection.
export const LIBRARY_KEYS: { label: string; route: Route }[] = [
  { label: "Albums", route: { name: "albums" } },
  { label: "Tracks", route: { name: "tracks" } },
  { label: "Artists", route: { name: "profiles" } },
  { label: "Mixes", route: { name: "mixes" } },
  { label: "History", route: { name: "history", tab: "recent" } },
  { label: "Purchases", route: { name: "purchases" } },
  { label: "Feed", route: { name: "feed" } },
];

/// The library key lit for a route, or null when the tray should not show
/// (detail pages keep the Library lamp but drop the tray).
export function libraryKeyOf(route: Route): string | null {
  const key = LIBRARY_KEYS.find((k) => k.route.name === route.name);
  return key ? key.label : null;
}
