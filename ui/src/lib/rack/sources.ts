import type { Route } from "../nav.svelte";

/// The selector unit's lit source keys (RK-D4). They mirror the default
/// shell's sidebar: Home, Explore (which holds search), Feed, History, the
/// collection (Library) and Settings (Setup).
export type Source = "home" | "explore" | "feed" | "history" | "library" | "settings";

const LIBRARY: Route["name"][] = [
  "playlists", "folder", "playlist", "albums", "tracks", "later", "profiles", "mixes", "purchases",
  "album", "artist", "artist-all", "mix",
];

export function sourceOf(route: Route): Source | null {
  switch (route.name) {
    case "home":
      return "home";
    case "explore":
    case "section":
    case "page":
      return "explore";
    case "feed":
      return "feed";
    case "history":
      return "history";
    case "settings":
      return "settings";
    default:
      return LIBRARY.includes(route.name) ? "library" : null;
  }
}

/// Sub-source keys shown above the page while browsing the collection; the
/// same entries as the sidebar's "My Collection" (including "Show more").
export const LIBRARY_KEYS: { label: string; route: Route; also?: Route["name"][] }[] = [
  { label: "Playlists", route: { name: "playlists" }, also: ["folder"] },
  { label: "Albums", route: { name: "albums" } },
  { label: "Tracks", route: { name: "tracks" } },
  { label: "Save for Later", route: { name: "later" } },
  { label: "Mixes & Radio", route: { name: "mixes" } },
  { label: "Profiles", route: { name: "profiles" } },
  { label: "Purchases", route: { name: "purchases" } },
];

/// The library key lit for a route, or null when the tray should not show
/// (detail pages keep the Library lamp but drop the tray).
export function libraryKeyOf(route: Route): string | null {
  const key = LIBRARY_KEYS.find((k) => k.route.name === route.name || k.also?.includes(route.name));
  return key ? key.label : null;
}
