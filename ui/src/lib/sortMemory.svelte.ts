import { MAX_LEVELS, SORT_KEYS, type SortKey, type SortLevel } from "./trackSort";

/// How long a page's track sort is remembered (MS-D2): until the page is
/// left, until the app closes (default), or always (persisted).
export type SortMemoryMode = "page" | "session" | "always";

export const MODE_KEY = "oxitide.track-sort-memory";
export const SORTS_KEY = "oxitide.track-sorts";
const MODES: readonly SortMemoryMode[] = ["page", "session", "always"];

export interface SortMemory {
  readonly mode: SortMemoryMode;
  setMode(mode: SortMemoryMode): void;
  /// The remembered sort for a page key (MS-D13), if any.
  get(key: string): SortLevel[] | undefined;
  set(key: string, levels: readonly SortLevel[]): void;
}

type MemoryStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

function validLevels(value: unknown): SortLevel[] | null {
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_LEVELS) return null;
  const levels: SortLevel[] = [];
  for (const entry of value) {
    if (typeof entry !== "object" || entry === null) return null;
    const { key, dir } = entry as Record<string, unknown>;
    if (!SORT_KEYS.includes(key as SortKey) || (dir !== "asc" && dir !== "desc")) return null;
    if (levels.some((level) => level.key === key)) return null;
    levels.push({ key: key as SortKey, dir });
  }
  return levels;
}

function loadSorts(storage: MemoryStorage): Map<string, SortLevel[]> {
  const sorts = new Map<string, SortLevel[]>();
  const raw = storage.getItem(SORTS_KEY);
  if (raw === null) return sorts;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return sorts;
    for (const [key, value] of Object.entries(parsed)) {
      const levels = validLevels(value);
      if (levels) sorts.set(key, levels);
    }
  } catch {
    // Corrupt JSON: start empty.
  }
  return sorts;
}

export function createSortMemory(storage: MemoryStorage): SortMemory {
  const stored = storage.getItem(MODE_KEY);
  const initial: SortMemoryMode = MODES.includes(stored as SortMemoryMode)
    ? (stored as SortMemoryMode)
    : "session";
  let mode = $state<SortMemoryMode>(initial);
  const sorts = initial === "always" ? loadSorts(storage) : new Map<string, SortLevel[]>();

  function persist() {
    if (mode !== "always") return;
    storage.setItem(SORTS_KEY, JSON.stringify(Object.fromEntries(sorts)));
  }

  return {
    get mode() {
      return mode;
    },
    setMode(next) {
      if (next === mode) return;
      mode = next;
      storage.setItem(MODE_KEY, next);
      if (next === "page") sorts.clear();
      if (next === "always") persist();
      else storage.removeItem(SORTS_KEY);
    },
    get(key) {
      if (mode === "page") return undefined;
      const levels = sorts.get(key);
      return levels ? levels.map((level) => ({ ...level })) : undefined;
    },
    set(key, levels) {
      if (mode === "page") return;
      if (levels.length === 0) sorts.delete(key);
      else sorts.set(key, levels.map((level) => ({ ...level })));
      persist();
    },
  };
}

let singleton: SortMemory | null = null;

/// App-wide memory bound to localStorage; `null` outside a browser (tests
/// that never pass a memory key).
export function sortMemory(): SortMemory | null {
  if (typeof window === "undefined" || !window.localStorage) return null;
  singleton ??= createSortMemory(window.localStorage);
  return singleton;
}
