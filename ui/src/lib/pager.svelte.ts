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

/// Drop every cached page; called when the signed-in account changes.
export function clearPagerCache() {
  cache.clear();
}

/// A scroll pager for a track list. `key` is `"favorites"` or
/// `"playlist:{uuid}"`; `load` fetches one page at `offset`. `dedupe` drops
/// ids already loaded (favorites can't repeat); playlists pass `false` so a
/// track that appears twice in the playlist stays twice in the queue.
export function createTrackPager(
  key: string,
  load: (offset: number, limit: number) => Promise<TrackPage>,
  pageSize = 100,
  dedupe = true,
): Pager {
  const cached = cache.get(key);
  let items = $state<Track[]>(cached ? [...cached.items] : []);
  let total = $state(cached?.total ?? 0);
  let done = $state(cached?.done ?? false);
  let loading = $state(false);
  let error = $state("");
  let nextOffset = cached?.nextOffset ?? 0;
  // The in-flight page request, shared by concurrent `loadMore` callers so
  // `ensureAll` awaits real progress instead of spinning on a resolved promise.
  let inflight: Promise<void> | null = null;

  function publish() {
    cache.set(key, { items: [...items], total, nextOffset, done });
  }

  function loadMore(): Promise<void> {
    if (inflight) return inflight;
    if (done) return Promise.resolve();
    inflight = (async () => {
      loading = true;
      error = "";
      const offset = nextOffset;
      try {
        const page = await load(offset, pageSize);
        const received = page.items.length;
        if (dedupe) {
          const seen = new Set(items.map((t) => t.id));
          // Dedupe only against already-loaded pages (offset overlap);
          // legitimate duplicate tracks within one page must survive.
          const fresh = page.items.filter((t) => !seen.has(t.id));
          items = [...items, ...fresh];
        } else {
          items = [...items, ...page.items];
        }
        const reportedTotal = page.total;
        // Advance by the raw page length so a page of duplicates cannot stall.
        nextOffset = offset + received;
        // Trust the reported total: TIDAL returns short pages mid-list, so a
        // short page only ends the walk when no total is reported (mirrors
        // `TidalClient::get_all_pages`); an empty page always ends it.
        done =
          received === 0 ||
          (reportedTotal > 0 ? nextOffset >= reportedTotal : received < pageSize);
        // A page that omits the total (serde default 0) must not erase a known one.
        if (reportedTotal > 0) total = reportedTotal;
        publish();
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
        inflight = null;
      }
    })();
    return inflight;
  }

  async function ensureAll() {
    while (!done && error === "") {
      await loadMore();
    }
  }

  async function refresh() {
    // Let an in-flight page finish first so it cannot republish into the
    // cache after the reset.
    if (inflight) await inflight;
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
