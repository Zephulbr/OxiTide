import { describe, expect, it, vi } from "vitest";
import type { Track, TrackPage } from "./api";
import { clearPagerCache, createTrackPager } from "./pager.svelte";

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

describe("track pager", () => {
  it("appends pages, dedupes by id, and stops when the offset reaches the total", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      offset === 0
        ? page([track(1), track(2), track(3)], 0, limit, 5)
        : page([track(3), track(4)], offset, limit, 5),
    );
    const pager = createTrackPager("append", load, 3);

    await pager.loadMore();
    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 3]);
    expect(pager.total).toBe(5);
    expect(pager.done).toBe(false);

    await pager.loadMore();
    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 3, 4]);
    expect(pager.done).toBe(true);
  });

  it("marks done when the offset reaches the total", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 2),
    );
    const pager = createTrackPager("offset-done", load, 2);

    await pager.loadMore();
    expect(pager.done).toBe(true);
    await pager.loadMore();
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("loads every page with ensureAll", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page(offset < 4 ? [track(offset + 1), track(offset + 2)] : [], offset, limit, 4),
    );
    const pager = createTrackPager("ensure-all", load, 2);

    await pager.ensureAll();
    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 3, 4]);
    expect(pager.done).toBe(true);
    expect(load.mock.calls.map(([offset]) => offset)).toEqual([0, 2]);
  });

  it("hydrates a new pager from the session cache and continues at the next offset", async () => {
    const loadA = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 4),
    );
    const a = createTrackPager("hydrate", loadA, 2);
    await a.loadMore();

    const loadB = vi.fn(async (offset: number, limit: number) =>
      page([track(3), track(4)], offset, limit, 4),
    );
    const b = createTrackPager("hydrate", loadB, 2);
    expect(b.items.map((t) => t.id)).toEqual([1, 2]);

    await b.loadMore();
    expect(loadB).toHaveBeenCalledWith(2, 2);
    expect(b.items.map((t) => t.id)).toEqual([1, 2, 3, 4]);
    expect(b.done).toBe(true);
  });

  it("refresh clears the cache and reloads from offset 0", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page(offset === 0 ? [track(1)] : [track(2)], offset, limit, 2),
    );
    const pager = createTrackPager("refresh", load, 1);
    await pager.loadMore();
    await pager.loadMore();
    expect(pager.items.map((t) => t.id)).toEqual([1, 2]);

    load.mockClear();
    await pager.refresh();

    expect(load).toHaveBeenCalledTimes(1);
    expect(load).toHaveBeenCalledWith(0, 1);
    expect(pager.items.map((t) => t.id)).toEqual([1]);
    expect(pager.done).toBe(false);
  });

  it("keeps loaded items on error and retries the same offset", async () => {
    let calls = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      calls += 1;
      if (calls === 2) throw new Error("boom");
      return page([track(calls)], offset, limit, 3);
    });
    const pager = createTrackPager("error-retry", load, 1);

    await pager.loadMore();
    await pager.loadMore();
    expect(pager.error).toContain("boom");
    expect(pager.items.map((t) => t.id)).toEqual([1]);

    await pager.loadMore();
    expect(pager.error).toBe("");
    expect(load).toHaveBeenLastCalledWith(1, 1);
    expect(pager.items.map((t) => t.id)).toEqual([1, 3]);
  });

  /// Review Focus: an error inside ensureAll must stop the loop, not spin.
  it("stops ensureAll on the first error instead of looping", async () => {
    const load = vi.fn(async () => {
      throw new Error("offline");
    });
    const pager = createTrackPager("error-stop", load, 2);

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
    const pager = createTrackPager("duplicate-page", load, 2);

    await pager.ensureAll();

    expect(pager.items.map((t) => t.id)).toEqual([1, 2]);
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
    const pager = createTrackPager("inflight", load, 2);

    const pending = pager.loadMore();
    const all = pager.ensureAll();
    // Resolve on a macrotask: a microtask-spinning `ensureAll` starves this
    // timer and the test times out, which is exactly the freeze being pinned.
    setTimeout(() => resolveFirst(page([track(1), track(2)], 0, 2, 4)), 0);

    await pending;
    await all;
    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 3, 4]);
    expect(pager.done).toBe(true);
  });

  it("clearPagerCache drops every cached page", async () => {
    const loadA = vi.fn(async (offset: number, limit: number) =>
      page([track(1)], offset, limit, 1),
    );
    const a = createTrackPager("clear-all", loadA, 1);
    await a.loadMore();

    clearPagerCache();

    const loadB = vi.fn(async (offset: number, limit: number) =>
      page([track(9)], offset, limit, 1),
    );
    const b = createTrackPager("clear-all", loadB, 1);
    expect(b.items).toEqual([]);
    await b.loadMore();
    expect(loadB).toHaveBeenCalledWith(0, 1);
    expect(b.items.map((t) => t.id)).toEqual([9]);
  });

  it("keeps legitimate duplicate tracks within one page", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(3), track(3)], offset, limit, 2),
    );
    const pager = createTrackPager("in-page-dupes", load, 2);

    await pager.loadMore();
    expect(pager.items.map((t) => t.id)).toEqual([3, 3]);
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
    const pager = createTrackPager("refresh-inflight", load, 1);

    const pending = pager.loadMore();
    const refreshing = pager.refresh();
    resolveFirst(page([track(1)], 0, 1, 1));

    await pending;
    await refreshing;
    expect(pager.items.map((t) => t.id)).toEqual([9]);
    expect(pager.done).toBe(true);
  });

  /// Root cause of the short-page stop: TIDAL returns short pages mid-list, so
  /// a short page must not end the walk while the reported total says more
  /// items remain (mirrors `TidalClient::get_all_pages`).
  it("continues past a short page while the total says more remain", async () => {
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset === 0) return page([track(1), track(2)], offset, limit, 10);
      if (offset === 2) return page([track(3), track(4), track(5)], offset, limit, 10);
      if (offset === 5) return page([track(6), track(7), track(8)], offset, limit, 10);
      return page([track(9), track(10)], offset, limit, 10);
    });
    const pager = createTrackPager("short-page", load, 3);

    await pager.ensureAll();

    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    expect(pager.done).toBe(true);
  });

  it("stops on an empty page when the total overstates the list", async () => {
    // The empty page carries no total (0): it must end the walk without
    // erasing the last known total.
    const load = vi.fn(async (offset: number, limit: number) =>
      offset === 0 ? page([track(1), track(2)], offset, limit, 10) : page([], offset, limit, 0),
    );
    const pager = createTrackPager("stale-total", load, 2);

    await pager.ensureAll();

    expect(pager.items.map((t) => t.id)).toEqual([1, 2]);
    expect(pager.total).toBe(10);
    expect(pager.done).toBe(true);
    expect(load).toHaveBeenCalledTimes(2);
  });

  it("uses the short-page rule when the server reports no total", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 0),
    );
    const pager = createTrackPager("no-total", load, 3);

    await pager.loadMore();

    expect(pager.items.map((t) => t.id)).toEqual([1, 2]);
    expect(pager.done).toBe(true);
  });

  /// Playlists may legitimately repeat a track; with dedupe off the queue must
  /// match the playlist exactly.
  it("keeps duplicate tracks across pages when dedupe is disabled", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      offset === 0
        ? page([track(1), track(2)], offset, limit, 4)
        : page([track(1), track(3)], offset, limit, 4),
    );
    const pager = createTrackPager("no-dedupe", load, 2, false);

    await pager.ensureAll();

    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 1, 3]);
    expect(pager.done).toBe(true);
  });
});
