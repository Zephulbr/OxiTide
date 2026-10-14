import type { PageItem } from "../lib/api";

/// The remote's hash routes (spec §6.5: no routing library).
export type RemoteRoute =
  | { name: "now" }
  | { name: "queue" }
  | { name: "search" }
  | { name: "home" }
  | { name: "explore" }
  | { name: "foryou" }
  | { name: "mixes" }
  | { name: "library" }
  | { name: "history" }
  | { name: "favorites"; kind: "tracks" | "albums" | "artists" }
  | { name: "playlists"; folder: string }
  | { name: "album"; id: number }
  | { name: "artist"; id: number }
  | { name: "playlist"; id: string }
  | { name: "mix"; id: string };

const SIMPLE = ["now", "queue", "search", "home", "explore", "foryou", "mixes", "library", "history"] as const;
const NUMERIC = /^\d+$/;
const TOKEN = /^[A-Za-z0-9_-]{1,80}$/;

export function parseHash(hash: string): RemoteRoute {
  const parts = hash.replace(/^#\/?/, "").split("/").filter(Boolean).map(decodeURIComponent);
  const [head = "now", arg] = parts;
  if ((SIMPLE as readonly string[]).includes(head) && parts.length === 1) {
    return { name: head as (typeof SIMPLE)[number] };
  }
  switch (head) {
    case "favorites":
      if (arg === "tracks" || arg === "albums" || arg === "artists") return { name: "favorites", kind: arg };
      break;
    case "playlists":
      return { name: "playlists", folder: arg && TOKEN.test(arg) ? arg : "root" };
    case "album":
    case "artist":
      if (arg && NUMERIC.test(arg)) return { name: head, id: Number(arg) };
      break;
    case "playlist":
    case "mix":
      if (arg && TOKEN.test(arg)) return { name: head, id: arg };
      break;
  }
  return { name: "now" };
}

export function hrefFor(route: RemoteRoute): string {
  switch (route.name) {
    case "favorites":
      return `#/favorites/${route.kind}`;
    case "playlists":
      return route.folder === "root" ? "#/playlists" : `#/playlists/${encodeURIComponent(route.folder)}`;
    case "album":
    case "artist":
    case "playlist":
    case "mix":
      return `#/${route.name}/${encodeURIComponent(String(route.id))}`;
    default:
      return `#/${route.name}`;
  }
}

/// Where a feed item leads (mirrors the desktop's `routeForPageItem`).
export function routeForItem(item: PageItem): RemoteRoute | null {
  if (!item.id) return null;
  const type = item.item_type.toUpperCase();
  if (type === "MIX" || type === "MIX_LIST") return { name: "mix", id: item.id };
  if (type.includes("ARTIST") && NUMERIC.test(item.id)) return { name: "artist", id: Number(item.id) };
  if ((type.includes("PLAYLIST") || type.includes("EDITORIAL")) && !NUMERIC.test(item.id)) {
    return { name: "playlist", id: item.id };
  }
  if (type.includes("ALBUM") && NUMERIC.test(item.id)) return { name: "album", id: Number(item.id) };
  return null;
}

/// Which bottom tab a route belongs to.
export function tabFor(route: RemoteRoute): "now" | "home" | "search" | "library" | "queue" {
  switch (route.name) {
    case "now":
    case "queue":
    case "search":
      return route.name;
    case "home":
    case "explore":
    case "foryou":
    case "mixes":
      return "home";
    default:
      return "library";
  }
}
