import { describe, expect, it, vi } from "vitest";
import type { Track } from "./api";
import { createTrackPager } from "./pager.svelte";
import { createTrackView, visibleIndexOf, warmTrackSearch } from "./trackView.svelte";

function track(id: number, title: string, duration = 100, artist = "A"): Track {
  return {
    id,
    title,
    duration,
    artists: [{ id: 1, name: artist }],
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  };
}

const SOURCE = [
  track(1, "Zebra", 300, "Muse"),
  track(2, "alpha", 100, "Radiohead"),
  track(3, "Beta", 200, "muse"),
];

function ids(tracks: Track[]): number[] {
  return tracks.map((t) => t.id);
}

describe("track view sort", () => {
  it("stays in source order until engaged", () => {
    const view = createTrackView(() => SOURCE);

    expect(view.sort).toBe("original");
    expect(view.engaged).toBe(false);
    expect(ids(view.items)).toEqual([1, 2, 3]);
  });

  it("cycles a column asc, desc, then back to original", () => {
    const view = createTrackView(() => SOURCE);

    view.toggleSort("title");
    expect(ids(view.items)).toEqual([2, 3, 1]);

    view.toggleSort("title");
    expect(ids(view.items)).toEqual([1, 3, 2]);

    view.toggleSort("title");
    expect(view.sort).toBe("original");
    expect(ids(view.items)).toEqual([1, 2, 3]);
  });

  it("switches columns at ascending and keeps a stable tie order", () => {
    const tied = [track(1, "Same", 200), track(2, "Same", 100), track(3, "Same", 300)];
    const view = createTrackView(() => tied);

    view.toggleSort("title");
    expect(ids(view.items)).toEqual([1, 2, 3]);

    view.toggleSort("duration");
    expect(ids(view.items)).toEqual([2, 1, 3]);
  });

  it("sorts durations numerically", () => {
    const view = createTrackView(() => SOURCE);

    view.toggleSort("duration");

    expect(ids(view.items)).toEqual([2, 3, 1]);
  });

  it("clearSort restores the original order without touching the filter", () => {
    const view = createTrackView(() => SOURCE);
    view.setFilter("a");
    view.toggleSort("title");

    view.clearSort();

    expect(view.sort).toBe("original");
    expect(view.filter).toBe("a");
  });
});

describe("track view filter", () => {
  it("matches title and artist names case-insensitively", () => {
    const view = createTrackView(() => SOURCE);

    view.setFilter("mus");

    expect(ids(view.items)).toEqual([1, 3]);
  });

  it("matches the title too", () => {
    const view = createTrackView(() => SOURCE);

    view.setFilter("alp");

    expect(ids(view.items)).toEqual([2]);
  });

  it("trims the filter and ignores an empty one", () => {
    const view = createTrackView(() => SOURCE);

    view.setFilter("  ");
    expect(view.engaged).toBe(false);

    view.setFilter("  beta ");
    expect(ids(view.items)).toEqual([3]);
  });

  it("sorts a filtered subset", () => {
    const view = createTrackView(() => SOURCE);
    view.setFilter("muse");
    view.toggleSort("title");

    expect(ids(view.items)).toEqual([3, 1]);
  });
});

describe("track view loading lifecycle", () => {
  it("calls ensureAll once when engagement begins", async () => {
    const ensureAll = vi.fn(async () => {});
    const view = createTrackView(() => SOURCE, ensureAll);

    view.setFilter("a");
    await Promise.resolve();
    view.setFilter("ab");

    expect(ensureAll).toHaveBeenCalledTimes(1);
    expect(view.preparing).toBe(false);
  });

  it("serves source order while preparing, then the sorted order", async () => {
    let release: () => void = () => {};
    const ensureAll = vi.fn(() => new Promise<void>((resolve) => (release = resolve)));
    const view = createTrackView(() => SOURCE, ensureAll);

    view.toggleSort("title");
    expect(view.preparing).toBe(true);
    expect(ids(view.items)).toEqual([1, 2, 3]);

    release();
    await Promise.resolve();
    await Promise.resolve();
    expect(view.preparing).toBe(false);
    expect(ids(view.items)).toEqual([2, 3, 1]);
  });

  it("reports an ensureAll failure and retries on demand", async () => {
    const ensureAll = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce(undefined);
    const view = createTrackView(() => SOURCE, ensureAll);

    view.toggleSort("title");
    await Promise.resolve();
    await Promise.resolve();
    expect(view.error).toBe("Error: offline");
    expect(ids(view.items)).toEqual([1, 2, 3]);

    view.retry();
    await Promise.resolve();
    await Promise.resolve();
    expect(view.error).toBe("");
    expect(ids(view.items)).toEqual([2, 3, 1]);
  });

  it("is ready immediately when no loader is supplied", () => {
    const view = createTrackView(() => SOURCE);

    view.toggleSort("title");

    expect(view.preparing).toBe(false);
    expect(ids(view.items)).toEqual([2, 3, 1]);
  });

  it("recovers from a real pager failure when retried", async () => {
    let calls = 0;
    const pager = createTrackPager(
      "view-retry",
      async (offset, limit) => {
        calls += 1;
        if (calls === 1) throw new Error("offline");
        return { items: [track(1, "A")], offset, limit, total: 1 };
      },
      { pageSize: 1, prefetch: false },
    );
    const ensureAll = async () => {
      await pager.ensureAll();
      if (pager.error) throw pager.error;
    };
    const view = createTrackView(() => pager.items, ensureAll);

    view.setFilter("a");
    await vi.waitFor(() => expect(view.error).toBe("Error: offline"));
    expect(view.items).toEqual([]);

    view.retry();

    await vi.waitFor(() => expect(view.error).toBe(""));
    expect(ids(view.items)).toEqual([1]);
  });

  it("clears preparing and error when the view disengages", async () => {
    const ensureAll = vi.fn(() => new Promise<void>(() => {}));
    const view = createTrackView(() => SOURCE, ensureAll);

    view.setFilter("a");
    expect(view.preparing).toBe(true);

    view.setFilter("");

    expect(view.engaged).toBe(false);
    expect(view.preparing).toBe(false);
    expect(view.error).toBe("");
  });
});

describe("track view sourceIndex", () => {
  it("maps a visible row back to its source position", () => {
    const view = createTrackView(() => SOURCE);
    view.toggleSort("duration");

    expect(ids(view.items)).toEqual([2, 3, 1]);
    expect(view.sourceIndex(0)).toBe(1);
    expect(view.sourceIndex(2)).toBe(0);
  });

  it("keeps duplicate tracks addressable by identity", () => {
    const source = [track(7, "Dup"), track(7, "Dup"), track(8, "Other")];
    const view = createTrackView(() => source);
    view.setFilter("dup");

    expect(view.items).toHaveLength(2);
    expect(view.sourceIndex(0)).toBe(0);
    expect(view.sourceIndex(1)).toBe(1);
  });

  it("returns -1 for an index outside the visible rows", () => {
    const view = createTrackView(() => SOURCE);

    expect(view.sourceIndex(9)).toBe(-1);
  });
});

describe("visibleIndexOf", () => {
  it("finds the captured track after the visible order changes", () => {
    const a = track(1, "A");
    const b = track(2, "B");

    expect(visibleIndexOf([b, a], a, 0)).toBe(1);
  });

  it("returns -1 when the captured track is filtered out", () => {
    const a = track(1, "A");

    expect(visibleIndexOf([], a, 0)).toBe(-1);
  });

  it("falls back to the captured index when there is no track", () => {
    expect(visibleIndexOf([], undefined, 3)).toBe(3);
  });
});

describe("track view sort keys", () => {
  it("does not call localeCompare per comparison", () => {
    const spy = vi.spyOn(String.prototype, "localeCompare");
    const view = createTrackView(() => SOURCE);
    view.toggleSort("title");
    const computed = ids(view.items);
    const calls = spy.mock.calls.length;
    spy.mockRestore();

    expect(computed).toEqual([2, 3, 1]);
    expect(calls).toBe(0);
  });

  it("keeps locale order for accent and case variants with stable ties", () => {
    const list = [track(1, "éclair"), track(2, "Echo"), track(3, "ÉCLAIR")];
    const view = createTrackView(() => list);

    view.toggleSort("title");

    expect(ids(view.items)).toEqual([2, 1, 3]);
  });

  it("maps sourceIndex without scanning the source", () => {
    const view = createTrackView(() => SOURCE);
    view.toggleSort("duration");
    const spy = vi.spyOn(Array.prototype, "indexOf");
    const mapped = view.sourceIndex(0);
    const calls = spy.mock.calls.length;
    spy.mockRestore();

    expect(mapped).toBe(1);
    expect(calls).toBe(0);
  });
});

describe("track view live updates", () => {
  it("keeps a filtered view consistent while a lazy page appends", async () => {
    const pager = createTrackPager(
      "filter-append",
      async (offset, limit) =>
        offset === 0
          ? { items: [track(1, "alpha"), track(2, "beta")], offset, limit, total: 4 }
          : { items: [track(3, "alphabet"), track(4, "gamma")], offset, limit, total: 4 },
      { pageSize: 2, prefetch: false },
    );
    await pager.loadMore();
    const view = createTrackView(() => pager.items, async () => {});

    view.setFilter("alp");
    await vi.waitFor(() => expect(view.preparing).toBe(false));
    expect(ids(view.items)).toEqual([1]);

    await pager.loadMore();

    expect(ids(view.items)).toEqual([1, 3]);
    expect(view.sourceIndex(1)).toBe(2);
  });
});

describe("warmTrackSearch", () => {
  it("fills the caches in chunks so a later filter does not rebuild them", async () => {
    const tracks = [track(1, "Alpha"), track(2, "Beta"), track(3, "Gamma")];
    const spy = vi.spyOn(String.prototype, "toLowerCase");
    const warm = warmTrackSearch(tracks, 2);
    await warm.done;
    const warmCalls = spy.mock.calls.length;

    const view = createTrackView(() => tracks);
    view.setFilter("alp");
    void view.items;
    const filterCalls = spy.mock.calls.length - warmCalls;
    spy.mockRestore();

    expect(warmCalls).toBeGreaterThanOrEqual(3);
    expect(filterCalls).toBe(1);
    expect(ids(view.items)).toEqual([1]);
  });

  it("stops warming when cancelled", async () => {
    const tracks = [track(1, "Alpha"), track(2, "Beta"), track(3, "Gamma")];
    const spy = vi.spyOn(String.prototype, "toLowerCase");
    const warm = warmTrackSearch(tracks, 1);
    warm.cancel();
    await warm.done;
    const calls = spy.mock.calls.length;
    spy.mockRestore();

    expect(calls).toBeLessThanOrEqual(2);
  });
});
