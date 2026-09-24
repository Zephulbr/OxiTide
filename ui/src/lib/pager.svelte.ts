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
}

export interface PagerOptions {
  /// Rows per first (and lazy) page. Default 100.
  pageSize?: number;
  /// Drop ids already loaded (favorites can't repeat); playlists pass `false`
  /// so a track that appears twice in the playlist stays twice in the queue.
  dedupe?: boolean;
  /// After the first page, fetch the remainder in a parallel burst. Default true.
  prefetch?: boolean;
  /// Rows per burst window. Default 1000.
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
  coveredRows: number;
  done: boolean;
}

/// Session cache: revisiting a view within one app run hydrates from here
/// instead of refetching. `refresh()` clears the key's entry.
const cache = new Map<string, CacheEntry>();

/// Drop every cached page; called when the signed-in account changes.
export function clearPagerCache() {
  cache.clear();
}

/// A pager for a track list. `key` is `"favorites"` or `"playlist:{uuid}"`;
/// `load` fetches one page at `offset`. The first page is small for a fast
/// paint; the remainder loads in parallel fixed windows (see `PagerOptions`).
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
  // `nextOffset` is the received-based stride used by lazy sequential pages;
  // `coveredRows` is the window end the burst has scanned. They differ because
  // the server omits unavailable rows from `items`.
  let nextOffset = cached?.nextOffset ?? 0;
  let coveredRows = cached?.coveredRows ?? 0;
  // The in-flight request task, shared by concurrent `loadMore` callers so
  // `ensureAll` awaits real progress instead of spinning on a resolved promise.
  let inflight: Promise<void> | null = null;

  function publish() {
    cache.set(key, { items: [...items], total, nextOffset, coveredRows, done });
  }

  /// Append a fetched page; dedupe only against already-loaded ids.
  function append(page: TrackPage) {
    if (dedupe) {
      const seen = new Set(items.map((t) => t.id));
      items = [...items, ...page.items.filter((t) => !seen.has(t.id))];
    } else {
      items = [...items, ...page.items];
    }
    if (page.total > 0) total = page.total;
  }

  /// One lazy/scroll page: the stride advances by what the server sent, so a
  /// silent server cap can only cause overlap (never skipped rows).
  async function loadSequentialPage() {
    const offset = nextOffset;
    const page = await load(offset, pageSize);
    append(page);
    const received = page.items.length;
    nextOffset = offset + received;
    coveredRows = Math.max(coveredRows, offset + pageSize);
    done =
      received === 0 ||
      (total > 0 ? nextOffset >= total : received < pageSize);
    publish();
  }

  /// First small page for a fast paint, then disjoint fixed windows fetched
  /// concurrently and applied in offset order. Offsets index the full list and
  /// the server honors the requested window, so windows cannot overlap or skip.
  async function runBurst() {
    const windows: number[] = [];
    for (let offset = coveredRows; offset < total; offset += prefetchWindow) {
      windows.push(offset);
    }
    for (let i = 0; i < windows.length; i += concurrency) {
      const group = windows.slice(i, i + concurrency);
      const pages = await Promise.all(
        group.map((offset) => load(offset, prefetchWindow)),
      );
      group.forEach((offset, index) => {
        append(pages[index]);
        coveredRows = Math.max(coveredRows, offset + prefetchWindow);
      });
      if (coveredRows >= total || pages.some((page) => page.items.length === 0)) {
        done = true;
        break;
      }
    }
    if (coveredRows >= total) done = true;
    if (done && total > 0) nextOffset = total;
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
    coveredRows = 0;
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
  };
}
