import { t, type MessageKey } from "./i18n.svelte";
import type { Route } from "./nav.svelte";

/// The page-name message for each route without its own title.
const ROUTE_KEYS = {
  home: "route.home",
  explore: "route.explore",
  feed: "route.feed",
  playlists: "route.playlists",
  albums: "route.albums",
  tracks: "route.tracks",
  later: "route.later",
  mixes: "route.mixes",
  profiles: "nav.profiles",
  purchases: "route.purchases",
  album: "route.album",
  playlist: "route.playlist",
  artist: "route.artist",
  "artist-all": "route.artist",
  mix: "route.mix",
  radio: "route.radio",
  settings: "route.settings",
} as const satisfies Partial<Record<Route["name"], MessageKey>>;

/// Window title per route. Entity names are not fetched, so detail routes
/// announce their kind; section/folder titles come from the route itself.
export function routeTitle(route: Route, nowPlaying = false): string {
  return t("app.windowTitle", { page: pageName(route, nowPlaying) });
}

function pageName(route: Route, nowPlaying: boolean): string {
  if (nowPlaying) return t("route.nowPlaying");
  switch (route.name) {
    case "history":
      return t(route.tab === "stats" ? "route.stats" : "route.history");
    case "folder":
      return route.label || t("route.folder");
    case "section":
    case "page":
      return route.title;
    default:
      return t(ROUTE_KEYS[route.name]);
  }
}
