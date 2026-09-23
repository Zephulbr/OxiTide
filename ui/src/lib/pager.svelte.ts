import type { Track, TrackPage } from "./api";

export interface Pager {
  readonly items: Track[];
  readonly total: number;
  readonly loading: boolean;
  readonly error: string;
  readonly done: boolean;
  loadMore(): Promise<void>;
  ensureAll(): Promise<void>;
  refresh(): Promise<void>;
}

interface CacheEntry {
  items: Track[];
  total: number;
  nextOffset: number;
  done: boolean;
}

/// Session cache: revisiting a view within one app run hydrates from here
/// instead of refetching. `refresh()` clears the key's entry.
const cache = new Map<string, CacheEntry>();

/// A scroll pager for a track list. `key` is `"favorites"` or
/// `"playlist:{uuid}"`; `load` fetches one page at `offset`.
export function createTrackPager(
  key: string,
  load: (offset: number, limit: number) => Promise<TrackPage>,
  pageSize = 100,
): Pager {
  const cached = cache.get(key);
  let items = $state<Track[]>(cached ? [...cached.items] : []);
  let total = $state(cached?.total ?? 0);
  let done = $state(cached?.done ?? false);
  let loading = $state(false);
  let error = $state("");
  let nextOffset = cached?.nextOffset ?? 0;

  function publish() {
    cache.set(key, { items: [...items], total, nextOffset, done });
  }

  async function loadMore() {
    if (loading || done) return;
    loading = true;
    error = "";
    const offset = nextOffset;
    try {
      const page = await load(offset, pageSize);
      const seen = new Set(items.map((t) => t.id));
      items = [...items, ...page.items.filter((t) => !seen.has(t.id))];
      total = page.total;
      // Advance by the raw page length so a page of duplicates cannot stall.
      nextOffset = offset + page.items.length;
      done = page.items.length < pageSize || nextOffset >= total;
      publish();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function ensureAll() {
    while (!done && error === "") {
      await loadMore();
    }
  }

  async function refresh() {
    cache.delete(key);
    items = [];
    total = 0;
    nextOffset = 0;
    done = false;
    error = "";
    await loadMore();
  }

  return {
    get items() { return items; },
    get total() { return total; },
    get loading() { return loading; },
    get error() { return error; },
    get done() { return done; },
    loadMore,
    ensureAll,
    refresh,
  };
}
