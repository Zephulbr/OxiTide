import { describe, expect, it, vi } from "vitest";
import type { Track, TrackPage } from "./api";
import { createTrackPager } from "./pager.svelte";

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
  it("appends pages, dedupes by id, and stops on a short page", async () => {
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
});
