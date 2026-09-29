export const HISTORY_KEY = "oxitide.search-history";

const MAX_ENTRIES = 8;

type HistoryStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

export interface SearchHistory {
  readonly entries: readonly string[];
  /// Trims, dedupes case-insensitively (newest casing wins), caps at 8.
  record(query: string): void;
  /// Case-insensitive substring filter; empty input returns every entry.
  matches(input: string): string[];
  clear(): void;
}

function load(storage: HistoryStorage): string[] {
  const raw = storage.getItem(HISTORY_KEY);
  if (raw === null) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((entry): entry is string => typeof entry === "string");
  } catch {
    return [];
  }
}

/// Persisted recent searches. The factory takes its storage so tests can pass
/// a fake; `searchHistory()` binds the app-wide singleton to localStorage.
export function createSearchHistory(storage: HistoryStorage): SearchHistory {
  let entries = $state<string[]>(load(storage));

  function persist() {
    storage.setItem(HISTORY_KEY, JSON.stringify(entries));
  }

  function record(query: string) {
    const trimmed = query.trim();
    if (trimmed.length === 0) return;
    const needle = trimmed.toLowerCase();
    entries = [trimmed, ...entries.filter((e) => e.toLowerCase() !== needle)].slice(0, MAX_ENTRIES);
    persist();
  }

  function matches(input: string): string[] {
    const needle = input.trim().toLowerCase();
    if (needle.length === 0) return [...entries];
    return entries.filter((entry) => entry.toLowerCase().includes(needle));
  }

  function clear() {
    entries = [];
    storage.removeItem(HISTORY_KEY);
  }

  return {
    get entries() {
      return entries;
    },
    record,
    matches,
    clear,
  };
}

let singleton: SearchHistory | null = null;

export function searchHistory(): SearchHistory {
  singleton ??= createSearchHistory(window.localStorage);
  return singleton;
}
