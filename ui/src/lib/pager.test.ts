import { describe, expect, it, vi } from "vitest";
import type { Track, TrackPage } from "./api";
import {
  cachedPagerItems,
  clearPagerCache,
  createTrackPager,
  invalidatePager,
  removePagerItem,
} from "./pager.svelte";

function track(id: number): Track {
  return {
    id,
    title: `T${id}`,
    duration: 1,
    artists: [],
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  };
}

function page(items: Track[], offset: number, limit: number, total: number): TrackPage {
  return { items, offset, limit, total };
}

function range(from: number, to: number): number[] {
  const out: number[] = [];
  for (let i = from; i <= to; i++) out.push(i);
  return out;
}

function ids(tracks: Track[]): number[] {
  return tracks.map((t) => t.id);
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

describe("track pager", () => {
  it("appends pages, dedupes by id, and stops when the offset reaches the total", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      offset === 0
        ? page([track(1), track(2), track(3)], 0, limit, 5)
        : page([track(3), track(4)], offset, limit, 5),
    );
    const pager = createTrackPager("append", load, { pageSize: 3, prefetch: false });

    await pager.loadMore();
    expect(ids(pager.items)).toEqual([1, 2, 3]);
    expect(pager.total).toBe(5);
    expect(pager.done).toBe(false);
    expect(pager.unavailable).toBe(0);

    await pager.loadMore();
    expect(ids(pager.items)).toEqual([1, 2, 3, 4]);
    expect(pager.done).toBe(true);
  });

  it("keeps legitimate duplicate tracks within one page", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(3), track(3)], offset, limit, 2),
    );
    const pager = createTrackPager("in-page-dupes", load, {
      pageSize: 2,
      prefetch: false,
    });

    await pager.loadMore();
    expect(ids(pager.items)).toEqual([3, 3]);
    expect(pager.done).toBe(true);
  });

  it("marks done when the offset reaches the total", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 2),
    );
    const pager = createTrackPager("offset-done", load, {
      pageSize: 2,
      prefetch: false,
    });

    await pager.loadMore();
    expect(pager.done).toBe(true);
    expect(pager.unavailable).toBe(0);
    await pager.loadMore();
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("loads every page with ensureAll", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page(offset < 4 ? [track(offset + 1), track(offset + 2)] : [], offset, limit, 4),
    );
    const pager = createTrackPager("ensure-all", load, { pageSize: 2, prefetch: false });

    await pager.ensureAll();
    expect(ids(pager.items)).toEqual([1, 2, 3, 4]);
    expect(pager.done).toBe(true);
    expect(load.mock.calls.map(([offset]) => offset)).toEqual([0, 2]);
  });

  it("hydrates a new pager from the session cache and continues at the next offset", async () => {
    const loadA = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 4),
    );
    const a = createTrackPager("hydrate", loadA, { pageSize: 2, prefetch: false });
    await a.loadMore();

    const loadB = vi.fn(async (offset: number, limit: number) =>
      page([track(3), track(4)], offset, limit, 4),
    );
    const b = createTrackPager("hydrate", loadB, { pageSize: 2, prefetch: false });
    expect(ids(b.items)).toEqual([1, 2]);

    await b.loadMore();
    expect(loadB).toHaveBeenCalledWith(2, 2);
    expect(ids(b.items)).toEqual([1, 2, 3, 4]);
    expect(b.done).toBe(true);
  });

  it("refresh clears the cache and reloads from offset 0", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page(offset === 0 ? [track(1)] : [track(2)], offset, limit, 2),
    );
    const pager = createTrackPager("refresh", load, { pageSize: 1, prefetch: false });
    await pager.loadMore();
    await pager.loadMore();
    expect(ids(pager.items)).toEqual([1, 2]);

    load.mockClear();
    await pager.refresh();

    expect(load).toHaveBeenCalledTimes(1);
    expect(load).toHaveBeenCalledWith(0, 1);
    expect(ids(pager.items)).toEqual([1]);
    expect(pager.done).toBe(false);
  });

  it("keeps loaded items on error and retries the same offset", async () => {
    let calls = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      calls += 1;
      if (calls === 2) throw new Error("boom");
      return page([track(calls)], offset, limit, 3);
    });
    const pager = createTrackPager("error-retry", load, { pageSize: 1, prefetch: false });

    await pager.loadMore();
    await pager.loadMore();
    expect(pager.error).toContain("boom");
    expect(ids(pager.items)).toEqual([1]);

    await pager.loadMore();
    expect(pager.error).toBe("");
    expect(load).toHaveBeenLastCalledWith(1, 1);
    expect(ids(pager.items)).toEqual([1, 3]);
  });

  /// Review Focus: an error inside ensureAll must stop the loop, not spin.
  it("stops ensureAll on the first error instead of looping", async () => {
    const load = vi.fn(async () => {
      throw new Error("offline");
    });
    const pager = createTrackPager("error-stop", load, { pageSize: 2, prefetch: false });

    await pager.ensureAll();

    expect(load).toHaveBeenCalledTimes(1);
    expect(pager.error).toContain("offline");
  });

  /// Review Focus: a full page of already-loaded tracks must still advance the
  /// offset, or ensureAll never terminates.
  it("advances the offset when a full page repeats already-loaded tracks", async () => {
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset < 4) return page([track(1), track(2)], offset, limit, 10);
      return page([], offset, limit, 10);
    });
    const pager = createTrackPager("duplicate-page", load, {
      pageSize: 2,
      prefetch: false,
    });

    await pager.ensureAll();

    expect(ids(pager.items)).toEqual([1, 2]);
    expect(pager.done).toBe(true);
    expect(load).toHaveBeenCalledTimes(3);
  });

  /// Review Focus: `ensureAll` must await an in-flight page, not spin on the
  /// `loading` short-circuit and starve the event loop.
  it("ensureAll waits for an in-flight page instead of spinning", async () => {
    let resolveFirst: (page: TrackPage) => void = () => {};
    const first = new Promise<TrackPage>((resolve) => {
      resolveFirst = resolve;
    });
    let calls = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      calls += 1;
      if (calls === 1) return first;
      return page(offset < 4 ? [track(offset + 1), track(offset + 2)] : [], offset, limit, 4);
    });
    const pager = createTrackPager("inflight", load, { pageSize: 2, prefetch: false });

    const pending = pager.loadMore();
    const all = pager.ensureAll();
    // Resolve on a macrotask: a microtask-spinning `ensureAll` starves this
    // timer and the test times out, which is exactly the freeze being pinned.
    setTimeout(() => resolveFirst(page([track(1), track(2)], 0, 2, 4)), 0);

    await pending;
    await all;
    expect(ids(pager.items)).toEqual([1, 2, 3, 4]);
    expect(pager.done).toBe(true);
  });

  it("clearPagerCache drops every cached page", async () => {
    const loadA = vi.fn(async (offset: number, limit: number) =>
      page([track(1)], offset, limit, 1),
    );
    const a = createTrackPager("clear-all", loadA, { pageSize: 1, prefetch: false });
    await a.loadMore();

    clearPagerCache();

    const loadB = vi.fn(async (offset: number, limit: number) =>
      page([track(9)], offset, limit, 1),
    );
    const b = createTrackPager("clear-all", loadB, { pageSize: 1, prefetch: false });
    expect(b.items).toEqual([]);
    await b.loadMore();
    expect(loadB).toHaveBeenCalledWith(0, 1);
    expect(ids(b.items)).toEqual([9]);
  });

  /// Playlists may legitimately repeat a track; with dedupe off the queue must
  /// match the playlist exactly.
  it("keeps duplicate tracks across pages when dedupe is disabled", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      offset === 0
        ? page([track(1), track(2)], offset, limit, 4)
        : page([track(1), track(3)], offset, limit, 4),
    );
    const pager = createTrackPager("no-dedupe", load, {
      pageSize: 2,
      prefetch: false,
      dedupe: false,
    });

    await pager.ensureAll();

    expect(ids(pager.items)).toEqual([1, 2, 1, 3]);
    expect(pager.done).toBe(true);
  });

  /// Root cause of the short-page stop: TIDAL returns short pages mid-list, so
  /// a short page must not end the walk while the reported total says more
  /// items remain (mirrors `TidalClient::get_all_pages`).
  it("continues past a short page while the total says more remain", async () => {
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset === 0) return page([track(1), track(2)], offset, limit, 10);
      if (offset === 3) return page([track(3), track(4), track(5)], offset, limit, 10);
      if (offset === 6) return page([track(6), track(7), track(8)], offset, limit, 10);
      return page([track(9), track(10)], offset, limit, 10);
    });
    const pager = createTrackPager("short-page", load, { pageSize: 3, prefetch: false });

    await pager.ensureAll();

    expect(ids(pager.items)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    expect(load.mock.calls.map(([offset]) => offset)).toEqual([0, 3, 6, 9]);
    expect(pager.done).toBe(true);
    expect(pager.unavailable).toBe(0);
  });

  it("uses the configured burst window", async () => {
    const total = 400;
    const load = vi.fn(async (offset: number, limit: number) =>
      page(range(offset + 1, Math.min(offset + limit, total)).map(track), offset, limit, total),
    );
    const pager = createTrackPager("window-100", load, {
      prefetchWindow: 100,
      concurrency: 4,
    });

    await pager.loadMore();

    expect(load.mock.calls.map(([offset]) => offset)).toEqual([0, 100, 200, 300]);
    expect(load.mock.calls.map(([, limit]) => limit)).toEqual([100, 100, 100, 100]);
    expect(ids(pager.items)).toEqual(range(1, 400));
    expect(pager.done).toBe(true);
  });

  it("stops on an empty page when the total overstates the list", async () => {
    // The empty page carries no total (0): it must end the walk without
    // erasing the last known total.
    const load = vi.fn(async (offset: number, limit: number) =>
      offset === 0 ? page([track(1), track(2)], offset, limit, 10) : page([], offset, limit, 0),
    );
    const pager = createTrackPager("stale-total", load, { pageSize: 2, prefetch: false });

    await pager.ensureAll();

    expect(ids(pager.items)).toEqual([1, 2]);
    expect(pager.total).toBe(10);
    expect(pager.unavailable).toBe(8);
    expect(pager.done).toBe(true);
    expect(load).toHaveBeenCalledTimes(2);
  });

  it("uses the short-page rule when the server reports no total", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 0),
    );
    const pager = createTrackPager("no-total", load, { pageSize: 3, prefetch: false });

    await pager.loadMore();

    expect(ids(pager.items)).toEqual([1, 2]);
    expect(pager.done).toBe(true);
  });

  /// Review Focus: `refresh` must discard an in-flight page rather than let it
  /// republish into the cache the refresh cleared.
  it("refresh discards an in-flight page instead of mixing it in", async () => {
    let resolveFirst: (page: TrackPage) => void = () => {};
    const first = new Promise<TrackPage>((resolve) => {
      resolveFirst = resolve;
    });
    let calls = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      calls += 1;
      if (calls === 1) return first;
      return page([track(9)], offset, limit, 1);
    });
    const pager = createTrackPager("refresh-inflight", load, {
      pageSize: 1,
      prefetch: false,
    });

    const pending = pager.loadMore();
    const refreshing = pager.refresh();
    resolveFirst(page([track(1)], 0, 1, 1));

    await pending;
    await refreshing;
    expect(ids(pager.items)).toEqual([9]);
    expect(pager.done).toBe(true);
  });

  it("bursts the remaining rows in fixed windows after a small first page", async () => {
    const total = 2366;
    const load = vi.fn(async (offset: number, limit: number) => {
      const idsForWindow =
        offset === 0
          ? range(1, 95)
          : offset === 100
            ? range(96, 1000)
            : offset === 1100
              ? range(1001, 1905)
              : range(1906, 2141);
      return page(idsForWindow.map(track), offset, limit, total);
    });
    const pager = createTrackPager("burst", load, {
      prefetchWindow: 1000,
      concurrency: 4,
    });

    await pager.loadMore();

    expect(load.mock.calls.map(([offset]) => offset)).toEqual([0, 100, 1100, 2100]);
    expect(load.mock.calls.map(([, limit]) => limit)).toEqual([100, 1000, 1000, 1000]);
    expect(ids(pager.items)).toEqual(range(1, 2141));
    expect(pager.total).toBe(2366);
    expect(pager.unavailable).toBe(225);
    expect(pager.done).toBe(true);
  });

  it("applies burst windows in offset order even when they resolve out of order", async () => {
    const total = 2366;
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset === 100) await delay(20);
      if (offset === 1100) await delay(1);
      const idsForWindow =
        offset === 0
          ? range(1, 95)
          : offset === 100
            ? range(96, 1000)
            : offset === 1100
              ? range(1001, 1905)
              : range(1906, 2141);
      return page(idsForWindow.map(track), offset, limit, total);
    });
    const pager = createTrackPager("burst-order", load, {
      prefetchWindow: 1000,
      concurrency: 4,
    });

    await pager.loadMore();

    expect(ids(pager.items)).toEqual(range(1, 2141));
  });

  it("limits burst concurrency", async () => {
    const total = 100 + 3 * 1000;
    let active = 0;
    let maxActive = 0;
    const gates: Array<() => void> = [];
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset === 0) return page([track(1)], offset, limit, total);
      active += 1;
      maxActive = Math.max(maxActive, active);
      await new Promise<void>((resolve) => gates.push(resolve));
      active -= 1;
      return page([track(offset + 1)], offset, limit, total);
    });
    const pager = createTrackPager("burst-concurrency", load, {
      prefetchWindow: 1000,
      concurrency: 2,
    });

    const pending = pager.loadMore();
    await vi.waitFor(() => expect(gates.length).toBe(2));
    expect(maxActive).toBe(2);
    expect(load.mock.calls.map(([offset]) => offset)).toEqual([0, 100, 1100]);

    gates[0]();
    gates[1]();
    await vi.waitFor(() => expect(gates.length).toBe(3));
    expect(maxActive).toBe(2);
    gates[2]();

    await pending;
    expect(pager.done).toBe(true);
  });

  it("stays lazy above the prefetch cap", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1)], offset, limit, 20_000),
    );
    const pager = createTrackPager("lazy-cap", load, { prefetchMaxTotal: 10_000 });

    await pager.loadMore();

    expect(load).toHaveBeenCalledTimes(1);
    expect(load).toHaveBeenCalledWith(0, 100);
    expect(pager.done).toBe(false);
    expect(pager.unavailable).toBe(0);
  });

  it("resumes the burst after a failed window", async () => {
    const total = 3000;
    let failOnce = true;
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset === 100 && failOnce) {
        failOnce = false;
        throw new Error("boom");
      }
      const idsForWindow =
        offset === 0
          ? range(1, 95)
          : offset === 100
            ? range(96, 995)
            : offset === 1100
              ? range(996, 1895)
              : range(1896, 2195);
      return page(idsForWindow.map(track), offset, limit, total);
    });
    const pager = createTrackPager("burst-retry", load, {
      prefetchWindow: 1000,
      concurrency: 4,
    });

    await pager.loadMore();
    expect(pager.error).toContain("boom");
    expect(pager.items.length).toBe(95);
    expect(pager.done).toBe(false);

    await pager.loadMore();
    expect(pager.error).toBe("");
    expect(pager.done).toBe(true);
    const loaded = ids(pager.items);
    expect(new Set(loaded).size).toBe(loaded.length);
    expect(loaded.length).toBe(95 + 900 + 900 + 300);
  });

  it("refresh reloads the whole list", async () => {
    const total = 2366;
    const load = vi.fn(async (offset: number, limit: number) => {
      const idsForWindow =
        offset === 0
          ? range(1, 95)
          : offset === 100
            ? range(96, 1000)
            : offset === 1100
              ? range(1001, 1905)
              : range(1906, 2141);
      return page(idsForWindow.map(track), offset, limit, total);
    });
    const pager = createTrackPager("refresh-burst", load, {
      prefetchWindow: 1000,
      concurrency: 4,
    });
    await pager.loadMore();
    expect(pager.items.length).toBe(2141);

    load.mockClear();
    await pager.refresh();

    expect(load.mock.calls.map(([offset]) => offset)).toEqual([0, 100, 1100, 2100]);
    expect(pager.items.length).toBe(2141);
    expect(pager.done).toBe(true);
  });

  it("removeItem drops the row, decrements total and nextOffset, and updates the cache", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2), track(3)], offset, limit, 3),
    );
    const pager = createTrackPager("remove-item", load, { pageSize: 3, prefetch: false });
    await pager.loadMore();
    expect(pager.done).toBe(true);

    pager.removeItem(2);

    expect(ids(pager.items)).toEqual([1, 3]);
    expect(pager.total).toBe(2);
    expect(pager.unavailable).toBe(0);

    const fresh = createTrackPager("remove-item", load, { pageSize: 3, prefetch: false });
    expect(ids(fresh.items)).toEqual([1, 3]);
    expect(fresh.total).toBe(2);
  });

  it("removeItem ignores ids that are not loaded", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1)], offset, limit, 1),
    );
    const pager = createTrackPager("remove-missing", load, { pageSize: 1, prefetch: false });
    await pager.loadMore();

    pager.removeItem(99);

    expect(ids(pager.items)).toEqual([1]);
    expect(pager.total).toBe(1);
  });

  it("removePagerItem corrects a cache entry with no mounted pager", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 2),
    );
    const a = createTrackPager("remove-cached", load, { pageSize: 2, prefetch: false });
    await a.loadMore();

    removePagerItem("remove-cached", 1);

    const b = createTrackPager("remove-cached", load, { pageSize: 2, prefetch: false });
    expect(ids(b.items)).toEqual([2]);
    expect(b.total).toBe(1);
  });

  it("invalidatePager forces the next mount to refetch", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1)], offset, limit, 1),
    );
    const a = createTrackPager("invalidate", load, { pageSize: 1, prefetch: false });
    await a.loadMore();

    invalidatePager("invalidate");

    const b = createTrackPager("invalidate", load, { pageSize: 1, prefetch: false });
    expect(b.items).toEqual([]);
  });

  it("cachedPagerItems peeks at a key's loaded rows", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 2),
    );
    expect(cachedPagerItems("peek")).toBeNull();

    const pager = createTrackPager("peek", load, { pageSize: 2, prefetch: false });
    await pager.loadMore();

    expect(ids(cachedPagerItems("peek") ?? [])).toEqual([1, 2]);
    expect(cachedPagerItems("missing")).toBeNull();
  });

  it("refreshes after a removal deferred behind an in-flight page", async () => {
    let resolveSecond: (page: TrackPage) => void = () => {};
    const second = new Promise<TrackPage>((resolve) => {
      resolveSecond = resolve;
    });
    let calls = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      calls += 1;
      if (calls === 1) return page([track(1), track(2)], offset, limit, 3);
      if (calls === 2) return second;
      // The post-delete server state: row 1 is gone.
      return page([track(2), track(3)], offset, limit, 2);
    });
    const pager = createTrackPager("remove-inflight", load, { pageSize: 2, prefetch: false });
    await pager.loadMore();
    expect(pager.done).toBe(false);

    const pending = pager.loadMore();
    pager.removeItem(1);
    // The in-flight window may already reflect the delete; its total must not
    // be decremented a second time.
    resolveSecond(page([track(3)], 2, 2, 2));
    await pending;

    expect(ids(pager.items)).toEqual([2, 3]);
    expect(pager.total).toBe(2);
    expect(pager.done).toBe(true);
  });
});

describe("pager moveItem", () => {
  it("moves a row down to its final index", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2), track(3), track(4)], offset, limit, 4),
    );
    const pager = createTrackPager("move-down", load, { pageSize: 4, prefetch: false });
    await pager.loadMore();

    pager.moveItem(0, 2);

    expect(ids(pager.items)).toEqual([2, 3, 1, 4]);
    expect(pager.total).toBe(4);
    expect(pager.done).toBe(true);
  });

  it("moves a row up and publishes the cache", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2), track(3)], offset, limit, 3),
    );
    const pager = createTrackPager("move-up", load, { pageSize: 3, prefetch: false });
    await pager.loadMore();

    pager.moveItem(2, 0);

    expect(ids(pager.items)).toEqual([3, 1, 2]);

    const rehydrated = createTrackPager("move-up", vi.fn(), {
      pageSize: 3,
      prefetch: false,
    });
    expect(ids(rehydrated.items)).toEqual([3, 1, 2]);
  });

  it("ignores equal or out-of-range indices", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 2),
    );
    const pager = createTrackPager("move-noop", load, { pageSize: 2, prefetch: false });
    await pager.loadMore();

    pager.moveItem(1, 1);
    pager.moveItem(-1, 0);
    pager.moveItem(0, 2);
    pager.moveItem(2, 0);

    expect(ids(pager.items)).toEqual([1, 2]);
  });
});

describe("pager ensureAll recovery", () => {
  it("retries the failed page on a later ensureAll", async () => {
    let calls = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      calls += 1;
      if (calls === 1) throw new Error("boom");
      return page([track(1), track(2)], offset, limit, 2);
    });
    const pager = createTrackPager("ensure-recover", load, { pageSize: 2, prefetch: false });

    await pager.ensureAll();
    expect(pager.error).toContain("boom");
    expect(ids(pager.items)).toEqual([]);

    await pager.ensureAll();

    expect(pager.error).toBe("");
    expect(ids(pager.items)).toEqual([1, 2]);
    expect(pager.done).toBe(true);
    expect(load).toHaveBeenCalledTimes(2);
  });
});

describe("pager in-place updates", () => {
  it("appends windows in place so the rendered array identity is stable", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(offset + 1), track(offset + 2)], offset, limit, 4),
    );
    const pager = createTrackPager("in-place-append", load, { pageSize: 2, prefetch: false });
    const first = pager.items;

    await pager.loadMore();
    await pager.loadMore();

    expect(pager.items).toBe(first);
    expect(ids(pager.items)).toEqual([1, 2, 3, 4]);
  });

  it("removes and moves rows in place", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2), track(3)], offset, limit, 3),
    );
    const pager = createTrackPager("in-place-edit", load, { pageSize: 3, prefetch: false });
    await pager.loadMore();
    const first = pager.items;

    pager.moveItem(0, 2);
    expect(pager.items).toBe(first);
    pager.removeItem(1);
    expect(pager.items).toBe(first);

    expect(ids(pager.items)).toEqual([2, 3]);
  });

  it("allows a removed row to come back from a later page", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      offset === 0
        ? page([track(1), track(2)], offset, limit, 4)
        : page([track(2), track(3)], offset, limit, 4),
    );
    const pager = createTrackPager("remove-then-readd", load, { pageSize: 2, prefetch: false });
    await pager.loadMore();

    pager.removeItem(2);
    await pager.loadMore();

    expect(ids(pager.items)).toEqual([1, 2, 3]);
  });

  it("reorders duplicate rows in place without corrupting the cache", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2), track(1)], offset, limit, 3),
    );
    const pager = createTrackPager("dup-move", load, {
      pageSize: 3,
      prefetch: false,
      dedupe: false,
    });
    await pager.loadMore();
    const first = pager.items;

    pager.moveItem(0, 2);

    expect(pager.items).toBe(first);
    expect(ids(pager.items)).toEqual([2, 1, 1]);
    const rehydrated = createTrackPager("dup-move", load, {
      pageSize: 3,
      prefetch: false,
      dedupe: false,
    });
    expect(ids(rehydrated.items)).toEqual([2, 1, 1]);
  });

  it("fetches the remainder in parallel above the burst cap on ensureAll", async () => {
    const total = 20_100;
    let active = 0;
    let maxActive = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset > 0) {
        active += 1;
        maxActive = Math.max(maxActive, active);
        await delay(5);
        active -= 1;
      }
      const to = Math.min(offset + limit, total);
      return page(range(offset + 1, to).map(track), offset, limit, total);
    });
    const pager = createTrackPager("ensure-parallel", load, { prefetchMaxTotal: 10_000 });

    await pager.ensureAll();

    expect(pager.done).toBe(true);
    expect(pager.items.length).toBe(total);
    expect(maxActive).toBe(4);
  });

  it("surfaces a burst failure from ensureAll as error, then resumes on retry", async () => {
    const total = 20_100;
    let failOnce = true;
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset > 100 && failOnce) {
        failOnce = false;
        throw new Error("boom");
      }
      const to = Math.min(offset + limit, total);
      return page(range(offset + 1, to).map(track), offset, limit, total);
    });
    const pager = createTrackPager("ensure-parallel-retry", load, {
      prefetchMaxTotal: 10_000,
    });

    await expect(pager.ensureAll()).resolves.toBeUndefined();
    expect(pager.error).toContain("boom");

    await pager.ensureAll();

    expect(pager.error).toBe("");
    expect(pager.done).toBe(true);
    expect(pager.items.length).toBe(total);
  });
});
