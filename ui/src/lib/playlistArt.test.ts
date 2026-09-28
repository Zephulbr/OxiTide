import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Track, TrackPage } from "./api";
import { api } from "./api";
import { clearPagerCache, createTrackPager } from "./pager.svelte";
import {
  clearPlaylistCovers,
  invalidatePlaylistCovers,
  playlistCovers,
  requestPlaylistCovers,
} from "./playlistArt.svelte";

vi.mock("./api", () => ({
  api: { getPlaylistTracksPage: vi.fn() },
}));

function track(id: number, cover: string | null): Track {
  return {
    id,
    title: `T${id}`,
    duration: 1,
    artists: [],
    album: cover ? { id, title: `A${id}`, cover } : null,
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  };
}

function page(items: Track[]): TrackPage {
  return { items, offset: 0, limit: 20, total: items.length };
}

function flush(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

describe("playlist art store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearPagerCache();
    clearPlaylistCovers();
  });

  it("caches the collage tiles and serves later requests without another fetch", async () => {
    vi.mocked(api.getPlaylistTracksPage).mockResolvedValue(
      page([track(1, "a"), track(2, "b"), track(3, "c"), track(4, "d"), track(5, "a")]),
    );

    requestPlaylistCovers("u1");
    await flush();

    expect(playlistCovers("u1")).toEqual(["a", "b", "c", "d"]);
    requestPlaylistCovers("u1");
    await flush();
    expect(api.getPlaylistTracksPage).toHaveBeenCalledTimes(1);
  });

  it("dedupes requests while one is in flight", async () => {
    let release: ((value: TrackPage) => void) | null = null;
    vi.mocked(api.getPlaylistTracksPage).mockImplementation(
      () =>
        new Promise<TrackPage>((resolve) => {
          release = resolve;
        }),
    );

    requestPlaylistCovers("u2");
    requestPlaylistCovers("u2");
    await flush();
    expect(api.getPlaylistTracksPage).toHaveBeenCalledTimes(1);

    release!(page([track(1, "a"), track(2, "b")]));
    await flush();
    expect(playlistCovers("u2")).toEqual(["a", "b", "a", "b"]);
  });

  it("uses the pager session cache without fetching", async () => {
    const load = vi.fn(async () => page([track(1, "a"), track(2, "b")]));
    const pager = createTrackPager("playlist:u3", load, { pageSize: 2, prefetch: false });
    await pager.loadMore();

    requestPlaylistCovers("u3");
    await flush();

    expect(api.getPlaylistTracksPage).not.toHaveBeenCalled();
    expect(playlistCovers("u3")).toEqual(["a", "b", "a", "b"]);
  });

  it("drops the cache on invalidate and refetches on the next request", async () => {
    vi.mocked(api.getPlaylistTracksPage).mockResolvedValue(page([track(1, "a"), track(2, "b")]));

    requestPlaylistCovers("u4");
    await flush();
    invalidatePlaylistCovers("u4");
    expect(playlistCovers("u4")).toBeNull();

    vi.mocked(api.getPlaylistTracksPage).mockResolvedValue(page([track(3, "c"), track(4, "d")]));
    requestPlaylistCovers("u4");
    await flush();
    expect(playlistCovers("u4")).toEqual(["c", "d", "c", "d"]);
    expect(api.getPlaylistTracksPage).toHaveBeenCalledTimes(2);
  });

  it("does not let a stale response repopulate an invalidated key", async () => {
    let release: ((value: TrackPage) => void) | null = null;
    vi.mocked(api.getPlaylistTracksPage).mockImplementation(
      () =>
        new Promise<TrackPage>((resolve) => {
          release = resolve;
        }),
    );

    requestPlaylistCovers("u5");
    invalidatePlaylistCovers("u5");
    release!(page([track(1, "a"), track(2, "b")]));
    await flush();

    expect(playlistCovers("u5")).toBeNull();
  });

  it("caches an empty result after a failure instead of retrying", async () => {
    vi.mocked(api.getPlaylistTracksPage).mockRejectedValue(new Error("offline"));

    requestPlaylistCovers("u6");
    await flush();
    expect(playlistCovers("u6")).toEqual([]);

    requestPlaylistCovers("u6");
    await flush();
    expect(api.getPlaylistTracksPage).toHaveBeenCalledTimes(1);
  });

  it("bounds concurrent fetches", async () => {
    const releases: ((value: TrackPage) => void)[] = [];
    vi.mocked(api.getPlaylistTracksPage).mockImplementation(
      () =>
        new Promise<TrackPage>((resolve) => {
          releases.push(resolve);
        }),
    );

    for (const id of ["a", "b", "c", "d", "e"]) requestPlaylistCovers(id);
    await flush();
    expect(api.getPlaylistTracksPage).toHaveBeenCalledTimes(4);

    releases[0](page([track(1, "a1"), track(2, "b1")]));
    await flush();
    expect(api.getPlaylistTracksPage).toHaveBeenCalledTimes(5);
  });
});
