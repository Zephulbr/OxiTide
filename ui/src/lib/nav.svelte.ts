import type { LaterKind, PageItem } from "./api";

export const NAV_KEY = "oxitide.nav";

type NavStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

/// Tabs of the artist catalog page; route values, so they live with the route.
export type ArtistTab = "tracks" | "albums" | "eps" | "appears" | "playlists" | "mixes";
const ARTIST_TABS: readonly ArtistTab[] = [
  "tracks",
  "albums",
  "eps",
  "appears",
  "playlists",
  "mixes",
];

/// Tabs of the History page; route values, so they live with the route.
export type HistoryTab = "recent" | "stats";
const HISTORY_TABS: readonly HistoryTab[] = ["recent", "stats"];

/// A TIDAL page a page link opens (`pages/genre_jazz`); Rust re-validates it.
export type PagePath = `pages/${string}`;
const PAGE_PATH = /^pages\/[A-Za-z0-9_-]+(\/[A-Za-z0-9_-]+)*$/;

export function isPagePath(value: unknown): value is PagePath {
  return typeof value === "string" && value.length <= 166 && PAGE_PATH.test(value);
}

/// Where a section's "View all" reloads it from: a fixed feed or a linked page.
export type SectionSource = "home" | "explore" | "forYou" | PagePath;

function isSectionSource(value: unknown): value is SectionSource {
  return value === "home" || value === "explore" || value === "forYou" || isPagePath(value);
}

/// The search Explore shows: its query and type chip live in the route, so
/// Back from a result returns to the results, not Explore's front page.
export type ExploreKind = "tracks" | "albums" | "artists" | "playlists";
const EXPLORE_KINDS: readonly ExploreKind[] = ["tracks", "albums", "artists", "playlists"];

/// The Save for Later page's type chip (absent: All).
const LATER_KINDS: readonly LaterKind[] = ["track", "album", "artist", "playlist"];

export type Route =
  | { name: "home" }
  | { name: "explore"; q?: string; kind?: ExploreKind }
  | { name: "feed" }
  | { name: "history"; tab: HistoryTab }
  | { name: "playlists" }
  | { name: "folder"; id: string; label?: string }
  | { name: "albums" }
  | { name: "tracks" }
  | { name: "later"; kind?: LaterKind }
  | { name: "mixes" }
  | { name: "profiles" }
  | { name: "purchases" }
  | { name: "album"; id: number }
  | { name: "playlist"; id: string }
  | { name: "artist"; id: number }
  | { name: "artist-all"; id: number; tab: ArtistTab }
  | { name: "mix"; id: string }
  | { name: "section"; source: SectionSource; title: string }
  | { name: "page"; path: PagePath; title: string }
  | { name: "settings" };

export interface Nav {
  readonly route: Route;
  readonly queueOpen: boolean;
  readonly nowPlayingOpen: boolean;
  readonly canBack: boolean;
  readonly canForward: boolean;
  /// Identity of the current history entry: unique per pushed entry for the
  /// app run (not persisted), kept by `replace`, revisited by back/forward.
  /// Keys per-entry view state such as the restored scroll position.
  readonly entry: number;
  goto(next: Route): void;
  /// Swap the current history entry without pushing a new one (tab switches).
  replace(next: Route): void;
  back(): void;
  forward(): void;
  toggleQueue(): void;
  toggleNowPlaying(): void;
  closeNowPlaying(): void;
  /// Return to home and drop the persisted history (logout).
  reset(): void;
}

export function sameRoute(a: Route, b: Route): boolean {
  if (a.name !== b.name) return false;
  if (a.name === "section" && b.name === "section") {
    return a.source === b.source && a.title === b.title;
  }
  if (a.name === "page" && b.name === "page") return a.path === b.path;
  if (a.name === "artist-all" && b.name === "artist-all") {
    return a.id === b.id && a.tab === b.tab;
  }
  if (a.name === "history" && b.name === "history") {
    return a.tab === b.tab;
  }
  if (a.name === "later" && b.name === "later") return a.kind === b.kind;
  if (a.name === "explore" && b.name === "explore") {
    return (a.q ?? "") === (b.q ?? "") && a.kind === b.kind;
  }
  if ("id" in a && "id" in b) return a.id === b.id;
  return true;
}

const NUMERIC_ID = /^\d+$/;

/// Map a page-feed item to a route only when the type and id shape agree.
/// Mix items open the mix page; page links open their TIDAL page; tracks,
/// videos, and unknown types stay inert.
export function routeForPageItem(item: PageItem): Route | null {
  if (isPagePath(item.api_path)) {
    return { name: "page", path: item.api_path, title: item.title || "Explore" };
  }
  if (item.id.length === 0) return null;
  const type = item.item_type.toUpperCase();
  if (type === "MIX" || type === "MIX_LIST") {
    return { name: "mix", id: item.id };
  }
  if (type.includes("ARTIST") && NUMERIC_ID.test(item.id)) {
    return { name: "artist", id: Number(item.id) };
  }
  if ((type.includes("PLAYLIST") || type.includes("EDITORIAL")) && !NUMERIC_ID.test(item.id)) {
    return { name: "playlist", id: item.id };
  }
  if (type.includes("ALBUM") && NUMERIC_ID.test(item.id)) {
    return { name: "album", id: Number(item.id) };
  }
  return null;
}

/// A persisted route checked field by field; anything malformed is null.
export function parseRoute(value: unknown): Route | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const record = value as Record<string, unknown>;
  switch (record.name) {
    case "home":
      return { name: "home" };
    case "explore": {
      const q = typeof record.q === "string" && record.q.trim().length > 0 ? record.q : undefined;
      if (!q) return { name: "explore" };
      const kind = record.kind;
      return typeof kind === "string" && EXPLORE_KINDS.includes(kind as ExploreKind)
        ? { name: "explore", q, kind: kind as ExploreKind }
        : { name: "explore", q };
    }
    case "feed":
      return { name: "feed" };
    case "history": {
      const tab = record.tab;
      if (typeof tab === "string" && HISTORY_TABS.includes(tab as HistoryTab)) {
        return { name: "history", tab: tab as HistoryTab };
      }
      return null;
    }
    case "playlists":
      return { name: "playlists" };
    case "albums":
      return { name: "albums" };
    case "tracks":
      return { name: "tracks" };
    case "later": {
      const kind = record.kind;
      return typeof kind === "string" && LATER_KINDS.includes(kind as LaterKind)
        ? { name: "later", kind: kind as LaterKind }
        : { name: "later" };
    }
    case "mixes":
      return { name: "mixes" };
    case "profiles":
      return { name: "profiles" };
    case "purchases":
      return { name: "purchases" };
    case "settings":
      return { name: "settings" };
    case "folder":
      if (typeof record.id !== "string" || record.id.length === 0) return null;
      return typeof record.label === "string"
        ? { name: "folder", id: record.id, label: record.label }
        : { name: "folder", id: record.id };
    case "playlist":
      return typeof record.id === "string" && record.id.length > 0
        ? { name: "playlist", id: record.id }
        : null;
    case "mix":
      return typeof record.id === "string" && record.id.length > 0
        ? { name: "mix", id: record.id }
        : null;
    case "section": {
      const source = record.source;
      if (isSectionSource(source)) {
        const title = record.title;
        if (typeof title === "string" && title.length > 0) {
          return { name: "section", source, title };
        }
      }
      return null;
    }
    case "page":
      return isPagePath(record.path) && typeof record.title === "string" && record.title.length > 0
        ? { name: "page", path: record.path, title: record.title }
        : null;
    case "album":
      return typeof record.id === "number" && Number.isInteger(record.id) && record.id > 0
        ? { name: "album", id: record.id }
        : null;
    case "artist":
      return typeof record.id === "number" && Number.isInteger(record.id) && record.id > 0
        ? { name: "artist", id: record.id }
        : null;
    case "artist-all": {
      const tab = record.tab;
      if (
        typeof record.id === "number" &&
        Number.isInteger(record.id) &&
        record.id > 0 &&
        typeof tab === "string" &&
        ARTIST_TABS.includes(tab as ArtistTab)
      ) {
        return { name: "artist-all", id: record.id, tab: tab as ArtistTab };
      }
      return null;
    }
    default:
      return null;
  }
}

interface StoredNav {
  history: Route[];
  index: number;
}

function load(storage: NavStorage): StoredNav | null {
  try {
    const raw = storage.getItem(NAV_KEY);
    if (raw === null) return null;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return null;
    const record = parsed as Record<string, unknown>;
    if (!Array.isArray(record.history) || record.history.length === 0) return null;
    const history: Route[] = [];
    for (const entry of record.history) {
      const route = parseRoute(entry);
      if (route === null) return null;
      history.push(route);
    }
    const index = record.index;
    if (
      typeof index !== "number" ||
      !Number.isInteger(index) ||
      index < 0 ||
      index >= history.length
    ) {
      return null;
    }
    return { history, index };
  } catch {
    return null;
  }
}

/// Route history. `goto` on the current route is a no-op; any new navigation
/// truncates forward entries. Queue visibility is orthogonal. With `storage`,
/// history and index persist across launches and hydrate on construction;
/// `reset` clears them (logout). Without it the nav stays in memory.
export function createNav(storage?: NavStorage): Nav {
  const stored = storage ? load(storage) : null;
  const initial: Route[] = stored ? stored.history : [{ name: "home" }];
  let history = $state<Route[]>(initial);
  let index = $state(stored ? stored.index : 0);
  let entries = $state<number[]>(initial.map((_, i) => i));
  let nextEntry = initial.length;
  let queueOpen = $state(false);
  let nowPlayingOpen = $state(false);

  function persist() {
    try {
      storage?.setItem(NAV_KEY, JSON.stringify({ history, index }));
    } catch {
      // A full or unavailable storage must not break navigation.
    }
  }

  function goto(next: Route) {
    if (sameRoute(history[index], next)) return;
    history = [...history.slice(0, index + 1), next];
    entries = [...entries.slice(0, index + 1), nextEntry++];
    index = history.length - 1;
    persist();
  }

  function replace(next: Route) {
    if (sameRoute(history[index], next)) return;
    history = history.map((route, i) => (i === index ? next : route));
    persist();
  }

  function back() {
    if (index > 0) {
      index -= 1;
      persist();
    }
  }

  function forward() {
    if (index < history.length - 1) {
      index += 1;
      persist();
    }
  }

  function reset() {
    history = [{ name: "home" }];
    entries = [nextEntry++];
    index = 0;
    try {
      storage?.removeItem(NAV_KEY);
    } catch {
      // Same as persist: storage failures are not navigation failures.
    }
  }

  return {
    get route() {
      return history[index];
    },
    get queueOpen() {
      return queueOpen;
    },
    get nowPlayingOpen() {
      return nowPlayingOpen;
    },
    get canBack() {
      return index > 0;
    },
    get canForward() {
      return index < history.length - 1;
    },
    get entry() {
      return entries[index];
    },
    goto,
    replace,
    back,
    forward,
    toggleQueue() {
      queueOpen = !queueOpen;
    },
    toggleNowPlaying() {
      nowPlayingOpen = !nowPlayingOpen;
    },
    closeNowPlaying() {
      nowPlayingOpen = false;
    },
    reset,
  };
}
