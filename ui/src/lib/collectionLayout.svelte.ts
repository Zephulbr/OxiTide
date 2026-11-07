import type { IconName } from "./components/Icon.svelte";

/// The sidebar's "My Collection" entries. Each key is also its route's name.
export type CollectionKey =
  | "playlists"
  | "albums"
  | "tracks"
  | "later"
  | "mixes"
  | "profiles"
  | "purchases";

export const COLLECTION_ITEMS: Record<CollectionKey, { label: string; icon: IconName }> = {
  playlists: { label: "Playlists", icon: "list-music" },
  albums: { label: "Albums", icon: "disc-3" },
  tracks: { label: "Tracks", icon: "music" },
  later: { label: "Save for Later", icon: "bookmark" },
  mixes: { label: "Mixes & Radio", icon: "disc-3" },
  profiles: { label: "Profiles", icon: "heart" },
  purchases: { label: "Purchases", icon: "copy" },
};

/// The default order; the first `DEFAULT_SPLIT` sit above the line.
export const COLLECTION_KEYS: readonly CollectionKey[] = [
  "playlists",
  "albums",
  "tracks",
  "later",
  "mixes",
  "profiles",
  "purchases",
];
export const DEFAULT_SPLIT = 4;

export const LAYOUT_KEY = "oxitide.collection-layout";

export interface CollectionLayoutPrefs {
  /// Every key, once, in sidebar order.
  order: CollectionKey[];
  /// How many of `order` sit above the line (always shown); the rest are
  /// under "Show more" (or the header's chevron when all are below).
  split: number;
  /// The hidden group is expanded.
  open: boolean;
}

/// An edit-mode row: an entry, or the line itself.
export type LayoutRow = CollectionKey | "line";

export function defaultLayout(): CollectionLayoutPrefs {
  return { order: [...COLLECTION_KEYS], split: DEFAULT_SPLIT, open: false };
}

/// Saved data made safe: unknown and repeated keys dropped, and a key the
/// save lacks (a page added since) appended below the line, so no entry is
/// ever lost.
export function normalizeLayout(raw: unknown): CollectionLayoutPrefs {
  const prefs = defaultLayout();
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) return prefs;
  const record = raw as Record<string, unknown>;
  if (!Array.isArray(record.order)) return prefs;
  const order: CollectionKey[] = [];
  for (const key of record.order) {
    if (COLLECTION_KEYS.includes(key as CollectionKey) && !order.includes(key as CollectionKey)) {
      order.push(key as CollectionKey);
    }
  }
  const kept = order.length;
  for (const key of COLLECTION_KEYS) if (!order.includes(key)) order.push(key);
  const split =
    typeof record.split === "number" && Number.isInteger(record.split)
      ? Math.max(0, Math.min(kept, record.split))
      : Math.min(kept, DEFAULT_SPLIT);
  return { order, split, open: record.open === true };
}

export function visibleKeys(prefs: CollectionLayoutPrefs): CollectionKey[] {
  return prefs.order.slice(0, prefs.split);
}

export function hiddenKeys(prefs: CollectionLayoutPrefs): CollectionKey[] {
  return prefs.order.slice(prefs.split);
}

/// The edit list: entries with the line at the split.
export function toRows(prefs: Pick<CollectionLayoutPrefs, "order" | "split">): LayoutRow[] {
  return [...prefs.order.slice(0, prefs.split), "line", ...prefs.order.slice(prefs.split)];
}

export function fromRows(rows: readonly LayoutRow[]): { order: CollectionKey[]; split: number } {
  const split = rows.indexOf("line");
  return {
    order: rows.filter((row): row is CollectionKey => row !== "line"),
    split: split < 0 ? rows.length : split,
  };
}

/// The rows with the one at `from` moved to index `to`.
export function moveRow(rows: readonly LayoutRow[], from: number, to: number): LayoutRow[] {
  const next = [...rows];
  if (from < 0 || from >= next.length) return next;
  const [row] = next.splice(from, 1);
  next.splice(Math.max(0, Math.min(next.length, to)), 0, row);
  return next;
}

export interface CollectionLayout {
  readonly prefs: CollectionLayoutPrefs;
  /// Moves an edit-list row (an entry or the line) from one index to another.
  move(from: number, to: number): void;
  setOpen(open: boolean): void;
  /// Back to the default order and line (the open state is kept).
  reset(): void;
}

type LayoutStorage = Pick<Storage, "getItem" | "setItem">;

function load(storage: LayoutStorage): CollectionLayoutPrefs {
  try {
    const raw = storage.getItem(LAYOUT_KEY);
    return raw === null ? defaultLayout() : normalizeLayout(JSON.parse(raw));
  } catch {
    // Corrupt JSON, or storage that throws: the defaults.
    return defaultLayout();
  }
}

/// The persisted "My Collection" arrangement, shared by the sidebar and the
/// Hi-Fi Rack's library tray. The factory takes its storage so tests can pass
/// a fake; `collectionLayout()` binds the app-wide singleton to localStorage.
export function createCollectionLayout(storage: LayoutStorage): CollectionLayout {
  let prefs = $state<CollectionLayoutPrefs>(load(storage));

  function persist() {
    try {
      storage.setItem(LAYOUT_KEY, JSON.stringify(prefs));
    } catch {
      // Full or blocked storage: the change still holds for this session.
    }
  }

  return {
    get prefs() {
      return prefs;
    },
    move(from, to) {
      if (from === to) return;
      prefs = { ...prefs, ...fromRows(moveRow(toRows(prefs), from, to)) };
      persist();
    },
    setOpen(open) {
      if (prefs.open === open) return;
      prefs = { ...prefs, open };
      persist();
    },
    reset() {
      prefs = { ...defaultLayout(), open: prefs.open };
      persist();
    },
  };
}

let singleton: CollectionLayout | null = null;

export function collectionLayout(): CollectionLayout {
  singleton ??= createCollectionLayout(storageOrMemory());
  return singleton;
}

/// Where localStorage is unavailable (some test environments): the layout
/// still works for the session, it just is not kept.
function storageOrMemory(): LayoutStorage {
  try {
    if (globalThis.localStorage) return globalThis.localStorage;
  } catch {
    // An embedder may throw on the property getter itself.
  }
  const items = new Map<string, string>();
  return { getItem: (key) => items.get(key) ?? null, setItem: (key, value) => void items.set(key, value) };
}
