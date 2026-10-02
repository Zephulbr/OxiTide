import type { PlaylistFilter, PlaylistSortKey, SortDirection } from "./playlistSort";

export const VIEW_KEY = "oxitide.playlist-view";

const DEFAULT_DIR: Record<PlaylistSortKey, SortDirection> = {
  recent: "desc",
  created: "desc",
  updated: "desc",
  alpha: "asc",
};

const SORT_KEYS: readonly PlaylistSortKey[] = ["recent", "created", "updated", "alpha"];
const FILTERS: readonly PlaylistFilter[] = ["all", "owned", "followed"];

export interface PlaylistViewPrefs {
  sort: PlaylistSortKey;
  dir: SortDirection;
  filter: PlaylistFilter;
}

export interface PlaylistView {
  readonly prefs: PlaylistViewPrefs;
  /// Picking the active key toggles direction; a new key uses its default.
  setSort(key: PlaylistSortKey): void;
  setFilter(filter: PlaylistFilter): void;
}

type ViewStorage = Pick<Storage, "getItem" | "setItem">;

function load(storage: ViewStorage): PlaylistViewPrefs {
  const prefs: PlaylistViewPrefs = { sort: "recent", dir: "desc", filter: "all" };
  const raw = storage.getItem(VIEW_KEY);
  if (raw === null) return prefs;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return prefs;
    const record = parsed as Record<string, unknown>;
    if (SORT_KEYS.includes(record.sort as PlaylistSortKey)) {
      prefs.sort = record.sort as PlaylistSortKey;
    }
    if (record.dir === "asc" || record.dir === "desc") prefs.dir = record.dir;
    if (FILTERS.includes(record.filter as PlaylistFilter)) {
      prefs.filter = record.filter as PlaylistFilter;
    }
  } catch {
    // Corrupt JSON falls back to the defaults above.
  }
  return prefs;
}

/// Persisted sidebar sort/filter prefs. The factory takes its storage so tests
/// can pass a fake; `playlistView()` binds the app-wide singleton to
/// localStorage.
export function createPlaylistView(storage: ViewStorage): PlaylistView {
  let prefs = $state<PlaylistViewPrefs>(load(storage));

  function persist() {
    storage.setItem(VIEW_KEY, JSON.stringify(prefs));
  }

  function setSort(key: PlaylistSortKey) {
    const dir =
      prefs.sort === key ? (prefs.dir === "asc" ? "desc" : "asc") : DEFAULT_DIR[key];
    prefs = { ...prefs, sort: key, dir };
    persist();
  }

  function setFilter(filter: PlaylistFilter) {
    prefs = { ...prefs, filter };
    persist();
  }

  return {
    get prefs() {
      return prefs;
    },
    setSort,
    setFilter,
  };
}

let singleton: PlaylistView | null = null;

export function playlistView(): PlaylistView {
  singleton ??= createPlaylistView(window.localStorage);
  return singleton;
}
