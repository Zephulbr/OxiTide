import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FavoriteIds, Track, TrackPage } from "./api";
import { api } from "./api";
import { createFavorites } from "./favorites.svelte";
import { clearPagerCache, createTrackPager } from "./pager.svelte";

vi.mock("./api", () => ({
  api: {
    getFavoriteIds: vi.fn(),
    setFavoriteTrack: vi.fn(),
    setFavoriteAlbum: vi.fn(),
    setFavoriteArtist: vi.fn(),
  },
}));

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

describe("favorites store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearPagerCache();
    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [], albums: [], artists: [] });
    vi.mocked(api.setFavoriteTrack).mockResolvedValue(undefined);
    vi.mocked(api.setFavoriteAlbum).mockResolvedValue(undefined);
    vi.mocked(api.setFavoriteArtist).mockResolvedValue(undefined);
  });

  it("hydrates the three sets from one call", async () => {
    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [1, 2], albums: [3], artists: [4] });
    const favorites = createFavorites();

    await favorites.hydrate();

    expect(favorites.hydrated).toBe(true);
    expect(favorites.hasTrack(1)).toBe(true);
    expect(favorites.hasTrack(9)).toBe(false);
    expect(favorites.hasAlbum(3)).toBe(true);
    expect(favorites.hasArtist(4)).toBe(true);
    expect(api.getFavoriteIds).toHaveBeenCalledTimes(1);
  });

  it("dedupes concurrent hydrates", async () => {
    const favorites = createFavorites();

    await Promise.all([favorites.hydrate(), favorites.hydrate()]);

    expect(api.getFavoriteIds).toHaveBeenCalledTimes(1);
  });

  it("leaves the store hollow after a failed hydrate and retries on the next call", async () => {
    vi.mocked(api.getFavoriteIds).mockRejectedValueOnce(new Error("offline"));
    const favorites = createFavorites();

    await favorites.hydrate();
    expect(favorites.hydrated).toBe(false);
    expect(favorites.error).toContain("offline");

    vi.mocked(api.getFavoriteIds).mockResolvedValueOnce({ tracks: [1], albums: [], artists: [] });
    await favorites.hydrate();
    expect(favorites.hydrated).toBe(true);
    expect(favorites.hasTrack(1)).toBe(true);
    expect(api.getFavoriteIds).toHaveBeenCalledTimes(2);
  });

  it("toggles optimistically and calls the matching command", async () => {
    const favorites = createFavorites();

    expect(await favorites.toggleTrack(5)).toBe(true);
    expect(api.setFavoriteTrack).toHaveBeenCalledWith(5, true);
    expect(favorites.hasTrack(5)).toBe(true);

    expect(await favorites.toggleTrack(5)).toBe(false);
    expect(api.setFavoriteTrack).toHaveBeenLastCalledWith(5, false);
    expect(favorites.hasTrack(5)).toBe(false);

    expect(await favorites.toggleAlbum(6)).toBe(true);
    expect(api.setFavoriteAlbum).toHaveBeenCalledWith(6, true);
    expect(await favorites.toggleArtist(7)).toBe(true);
    expect(api.setFavoriteArtist).toHaveBeenCalledWith(7, true);
  });

  it("reverts a failed toggle and surfaces the error", async () => {
    vi.mocked(api.setFavoriteTrack).mockRejectedValueOnce(new Error("rate limited"));
    const favorites = createFavorites();

    expect(await favorites.toggleTrack(5)).toBeNull();
    expect(favorites.hasTrack(5)).toBe(false);
    expect(favorites.error).toContain("rate limited");

    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [5], albums: [], artists: [] });
    await favorites.hydrate();
    vi.mocked(api.setFavoriteTrack).mockRejectedValueOnce(new Error("boom"));

    expect(await favorites.toggleTrack(5)).toBeNull();
    expect(favorites.hasTrack(5)).toBe(true);
  });

  it("ignores a repeat click while a toggle is pending", async () => {
    let release: (() => void) | null = null;
    vi.mocked(api.setFavoriteTrack).mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          release = resolve;
        }),
    );
    const favorites = createFavorites();

    const first = favorites.toggleTrack(5);
    expect(await favorites.toggleTrack(5)).toBe(true);
    expect(api.setFavoriteTrack).toHaveBeenCalledTimes(1);

    release!();
    await first;
    expect(favorites.hasTrack(5)).toBe(true);
  });

  it("drops an unfavorited track from the cached favorites list and bumps the revision", async () => {
    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [1, 2], albums: [], artists: [] });
    const load = vi.fn(
      async (offset: number, limit: number): Promise<TrackPage> => ({
        items: [track(1), track(2)],
        offset,
        limit,
        total: 2,
      }),
    );
    const pager = createTrackPager("favorites", load, { pageSize: 2, prefetch: false });
    await pager.loadMore();

    const favorites = createFavorites();
    await favorites.hydrate();
    expect(await favorites.toggleTrack(1)).toBe(false);

    expect(favorites.trackRevision).toBe(1);
    expect(favorites.lastUnfavoritedTrackId).toBe(1);
    const fresh = createTrackPager("favorites", load, { pageSize: 2, prefetch: false });
    expect(fresh.items.map((t) => t.id)).toEqual([2]);
    expect(fresh.total).toBe(1);
  });

  it("does not repopulate after a reset during an in-flight hydrate", async () => {
    let resolveIds: ((ids: FavoriteIds) => void) | null = null;
    vi.mocked(api.getFavoriteIds).mockImplementation(
      () =>
        new Promise<FavoriteIds>((resolve) => {
          resolveIds = resolve;
        }),
    );
    const favorites = createFavorites();

    const hydrating = favorites.hydrate();
    favorites.reset();
    resolveIds!({ tracks: [1], albums: [], artists: [] });
    await hydrating;

    expect(favorites.hydrated).toBe(false);
    expect(favorites.hasTrack(1)).toBe(false);
  });

  it("clears everything on reset", async () => {
    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [1], albums: [2], artists: [3] });
    const favorites = createFavorites();
    await favorites.hydrate();
    vi.mocked(api.setFavoriteTrack).mockRejectedValueOnce(new Error("boom"));
    await favorites.toggleTrack(9);

    favorites.reset();

    expect(favorites.hydrated).toBe(false);
    expect(favorites.error).toBeNull();
    expect(favorites.hasTrack(1)).toBe(false);
    expect(favorites.hasAlbum(2)).toBe(false);
    expect(favorites.hasArtist(3)).toBe(false);
    expect(favorites.trackRevision).toBe(0);
    expect(favorites.lastUnfavoritedTrackId).toBeNull();
  });
});
