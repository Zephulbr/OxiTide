import type { Route } from "./nav.svelte";

/// Window title per route. Entity names are not fetched, so detail routes
/// announce their kind; section/folder titles come from the route itself.
export function routeTitle(route: Route, nowPlaying = false): string {
  if (nowPlaying) return "Now playing · OxiTide";
  switch (route.name) {
    case "home":
      return "Home · OxiTide";
    case "explore":
      return "Explore · OxiTide";
    case "feed":
      return "Feed · OxiTide";
    case "history":
      return route.tab === "stats" ? "Stats · OxiTide" : "History · OxiTide";
    case "playlists":
      return "Playlists · OxiTide";
    case "folder":
      return `${route.label || "Folder"} · OxiTide`;
    case "albums":
      return "Albums · OxiTide";
    case "tracks":
      return "Tracks · OxiTide";
    case "mixes":
      return "Mixes · OxiTide";
    case "profiles":
      return "Artists · OxiTide";
    case "purchases":
      return "Purchases · OxiTide";
    case "album":
      return "Album · OxiTide";
    case "playlist":
      return "Playlist · OxiTide";
    case "artist":
    case "artist-all":
      return "Artist · OxiTide";
    case "mix":
      return "Mix · OxiTide";
    case "section":
      return `${route.title} · OxiTide`;
    case "settings":
      return "Settings · OxiTide";
  }
}
