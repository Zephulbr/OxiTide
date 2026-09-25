import { describe, expect, it, vi } from "vitest";
import type { Track } from "./api";
import { createTrackView } from "./trackView.svelte";

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
