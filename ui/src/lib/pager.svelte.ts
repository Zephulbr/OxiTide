import type { Track, TrackPage } from "./api";

export interface Pager {
  readonly items: Track[];
  readonly total: number;
  readonly loading: boolean;
  readonly error: string;
  readonly done: boolean;
  /// Items the server counts in `total` but never returns (unavailable
  /// tracks); non-zero only once the walk is done.
  readonly unavailable: number;
  loadMore(): Promise<void>;
  ensureAll(): Promise<void>;
  refresh(): Promise<void>;
  /// Drop one row locally (an unfavorited track); no-op when absent.
  removeItem(id: number): void;
  /// Reorder one loaded row (drag/arrow move). `to` is the final index within
  /// the loaded rows; equal or out-of-range indices are a no-op. Totals and
  /// offsets are unchanged, so this is safe while a page fetch is in flight.
  moveItem(from: number, to: number): void;
}

export interface PagerOptions {
  /// Rows per first/lazy page. Default 100.
  pageSize?: number;
  /// Drop ids already loaded (favorites can't repeat); playlists pass `false`
  /// so a track that appears twice in the playlist stays twice in the queue.
  dedupe?: boolean;
  /// After the first page, fetch the remainder in a parallel burst. Default true.
  prefetch?: boolean;
  /// Rows per burst window. Default 1000; playlists pass 100 (the size the
  /// playlist endpoint is known to honor).
  prefetchWindow?: number;
  /// Above this reported total the pager stays lazy (scroll-paged). Default 10_000.
  prefetchMaxTotal?: number;
  /// Burst windows in flight. Default 4.
  concurrency?: number;
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

/// Drop one row from a cached list when no pager instance is mounted.
/// `nextOffset` is the first unscanned row, so removing a scanned row shifts
/// it down by one.
export function removePagerItem(key: string, id: number) {
  const entry = cache.get(key);
  if (!entry) return;
  if (!entry.items.some((t) => t.id === id)) return;
  entry.items = entry.items.filter((t) => t.id !== id);
  if (entry.total > 0) entry.total -= 1;
  entry.nextOffset = Math.max(0, entry.nextOffset - 1);
}

/// Forget a cached list so the next mount refetches it.
export function invalidatePager(key: string) {
  cache.delete(key);
}

/// A pager for a track list. `key` is `"favorites"` or `"playlist:{uuid}"`;
/// `load` fetches one window at `offset`. The first page is small for a fast
/// paint; the remainder loads in parallel fixed windows (see `PagerOptions`).
///
/// Offsets index the full list (unavailable rows are omitted from `items` but
/// still counted by `total`), so every request advances by its requested
/// window size: disjoint windows cannot overlap or skip rows.
export function createTrackPager(
  key: string,
  load: (offset: number, limit: number) => Promise<TrackPage>,
  options: PagerOptions = {},
): Pager {
  const pageSize = options.pageSize ?? 100;
  const dedupe = options.dedupe ?? true;
  const prefetch = options.prefetch ?? true;
  const prefetchWindow = options.prefetchWindow ?? 1000;
  const prefetchMaxTotal = options.prefetchMaxTotal ?? 10_000;
  const concurrency = options.concurrency ?? 4;

  const cached = cache.get(key);
  let items = $state<Track[]>(cached ? [...cached.items] : []);
  let total = $state(cached?.total ?? 0);
  let done = $state(cached?.done ?? false);
  let loading = $state(false);
  let error = $state("");
  /// The first row not yet scanned (window end); also the next request offset.
  let nextOffset = cached?.nextOffset ?? 0;
  // The in-flight request task, shared by concurrent `loadMore` callers so
  // `ensureAll` awaits real progress instead of spinning on a resolved promise.
  let inflight: Promise<void> | null = null;
  /// A removal requested while a page fetch is in flight cannot be applied
  /// locally (the window's offsets and total may already reflect it), so the
  /// list is refetched once the fetch settles.
  let needsResync = false;

  function publish() {
    cache.set(key, { items: [...items], total, nextOffset, done });
  }

  /// Append a fetched window and advance past it.
  function applyWindow(page: TrackPage, offset: number, limit: number) {
    if (dedupe) {
      const seen = new Set(items.map((t) => t.id));
      items = [...items, ...page.items.filter((t) => !seen.has(t.id))];
    } else {
      items = [...items, ...page.items];
    }
    if (page.total > 0) total = page.total;
    const received = page.items.length;
    nextOffset = offset + limit;
    done =
      received === 0 ||
      (total > 0 ? nextOffset >= total : received < limit);
  }

  function applyRemoval(id: number) {
    if (!items.some((t) => t.id === id)) return;
    items = items.filter((t) => t.id !== id);
    if (total > 0) total -= 1;
    nextOffset = Math.max(0, nextOffset - 1);
    publish();
  }

  function removeItem(id: number) {
    if (inflight) {
      needsResync = true;
      return;
    }
    applyRemoval(id);
  }

  function moveItem(from: number, to: number) {
    if (from === to) return;
    if (from < 0 || from >= items.length) return;
    if (to < 0 || to >= items.length) return;
    const next = [...items];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    items = next;
    publish();
  }

  async function loadSequentialPage() {
    const offset = nextOffset;
    const page = await load(offset, pageSize);
    applyWindow(page, offset, pageSize);
    publish();
  }

  /// Disjoint fixed windows fetched concurrently and applied in offset order.
  async function runBurst() {
    const windows: number[] = [];
    for (let offset = nextOffset; offset < total; offset += prefetchWindow) {
      windows.push(offset);
    }
    for (let i = 0; i < windows.length; i += concurrency) {
      const group = windows.slice(i, i + concurrency);
      const pages = await Promise.all(
        group.map((offset) => load(offset, prefetchWindow)),
      );
      group.forEach((offset, index) => applyWindow(pages[index], offset, prefetchWindow));
      if (done || pages.some((page) => page.items.length === 0)) {
        done = true;
        break;
      }
    }
    if (nextOffset >= total) done = true;
    publish();
  }

  async function makeProgress() {
    let loadedFirstPage = false;
    if (items.length === 0 && nextOffset === 0) {
      await loadSequentialPage();
      loadedFirstPage = true;
    }
    if (done) return;
    if (prefetch && total > 0 && total <= prefetchMaxTotal) {
      await runBurst();
    } else if (!loadedFirstPage) {
      await loadSequentialPage();
    }
  }

  function loadMore(): Promise<void> {
    if (inflight) return inflight;
    if (done) return Promise.resolve();
    inflight = (async () => {
      loading = true;
      error = "";
      try {
        await makeProgress();
      } catch (e) {
        error = String(e);
        // Keep whatever windows completed before the failure.
        publish();
      } finally {
        loading = false;
        inflight = null;
        if (needsResync) {
          needsResync = false;
          await refresh();
        }
      }
    })();
    return inflight;
  }

  async function ensureAll() {
    // A prior failure leaves `error` set; clear it so a later call (the
    // header Retry) drives the failed page instead of returning at once.
    if (error !== "") error = "";
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
    get unavailable() {
      return done && total > items.length ? total - items.length : 0;
    },
    loadMore,
    ensureAll,
    refresh,
    removeItem,
    moveItem,
  };
}
