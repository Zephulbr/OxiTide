import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FavoriteIds, Track, TrackPage } from "./api";
import { api } from "./api";
import { createFavorites } from "./favorites.svelte";
import { clearPagerCache, createTrackPager } from "./pager.svelte";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

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

interface FakeToasts extends Toasts {
  readonly calls: { message: string; severity: ToastSeverity }[];
}

function fakeToasts(): FakeToasts {
  const calls: { message: string; severity: ToastSeverity }[] = [];
  return {
    get toasts() {
      return [];
    },
    show(message, options) {
      calls.push({ message, severity: options?.severity ?? "info" });
      return calls.length;
    },
    dismiss() {},
    get calls() {
      return calls;
    },
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

  it("applies a change made on a phone and keeps it over a stale hydration", async () => {
    let resolve!: (ids: FavoriteIds) => void;
    vi.mocked(api.getFavoriteIds).mockReturnValue(new Promise((r) => (resolve = r)));
    const favorites = createFavorites(fakeToasts());
    const hydrating = favorites.hydrate();
    favorites.applyExternal("track", 5, true);
    favorites.applyExternal("album", 9, true);
    expect(favorites.hasTrack(5)).toBe(true);
    resolve({ tracks: [], albums: [9], artists: [] });
    await hydrating;
    expect(favorites.hasTrack(5)).toBe(true);
    favorites.applyExternal("album", 9, false);
    expect(favorites.hasAlbum(9)).toBe(false);
    const before = favorites.trackRevision;
    favorites.applyExternal("track", 5, false);
    expect(favorites.hasTrack(5)).toBe(false);
    expect(favorites.trackRevision).toBe(before + 1);
  });

  it("ignores the echo of an in-flight local toggle", async () => {
    let finish!: () => void;
    vi.mocked(api.setFavoriteTrack).mockReturnValue(new Promise((r) => (finish = () => r(undefined))));
    const favorites = createFavorites(fakeToasts());
    const toggling = favorites.toggleTrack(3);
    favorites.applyExternal("track", 3, false);
    expect(favorites.hasTrack(3)).toBe(true);
    finish();
    await toggling;
    expect(favorites.hasTrack(3)).toBe(true);
  });

  it("hydrates the three sets from one call", async () => {
    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [1, 2], albums: [3], artists: [4] });
    const favorites = createFavorites(fakeToasts());

    await favorites.hydrate();

    expect(favorites.hydrated).toBe(true);
    expect(favorites.hasTrack(1)).toBe(true);
    expect(favorites.hasTrack(9)).toBe(false);
    expect(favorites.hasAlbum(3)).toBe(true);
    expect(favorites.hasArtist(4)).toBe(true);
    expect(api.getFavoriteIds).toHaveBeenCalledTimes(1);
  });

  it("dedupes concurrent hydrates", async () => {
    const favorites = createFavorites(fakeToasts());

    await Promise.all([favorites.hydrate(), favorites.hydrate()]);

    expect(api.getFavoriteIds).toHaveBeenCalledTimes(1);
  });

  it("leaves the store hollow after a failed hydrate and retries on the next call", async () => {
    vi.mocked(api.getFavoriteIds).mockRejectedValueOnce(new Error("offline"));
    const toasts = fakeToasts();
    const favorites = createFavorites(toasts);

    await favorites.hydrate();
    expect(favorites.hydrated).toBe(false);
    expect(toasts.calls).toEqual([{ message: "Error: offline", severity: "error" }]);

    vi.mocked(api.getFavoriteIds).mockResolvedValueOnce({ tracks: [1], albums: [], artists: [] });
    await favorites.hydrate();
    expect(favorites.hydrated).toBe(true);
    expect(favorites.hasTrack(1)).toBe(true);
    expect(api.getFavoriteIds).toHaveBeenCalledTimes(2);
  });

  it("toggles optimistically and calls the matching command", async () => {
    const favorites = createFavorites(fakeToasts());

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

  it("reverts a failed toggle and reports a mapped error toast", async () => {
    vi.mocked(api.setFavoriteTrack).mockRejectedValueOnce("rate limited");
    const toasts = fakeToasts();
    const favorites = createFavorites(toasts);

    expect(await favorites.toggleTrack(5)).toBeNull();
    expect(favorites.hasTrack(5)).toBe(false);
    expect(toasts.calls).toEqual([
      {
        message: "TIDAL is rate-limiting requests — try again in a moment",
        severity: "error",
      },
    ]);

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
    const favorites = createFavorites(fakeToasts());

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

    const favorites = createFavorites(fakeToasts());
    await favorites.hydrate();
    expect(await favorites.toggleTrack(1)).toBe(false);

    expect(favorites.trackRevision).toBe(1);
    expect(favorites.pendingRemovalsToDrop(new Set([1, 2]))).toEqual([1]);
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
    const favorites = createFavorites(fakeToasts());

    const hydrating = favorites.hydrate();
    favorites.reset();
    resolveIds!({ tracks: [1], albums: [], artists: [] });
    await hydrating;

    expect(favorites.hydrated).toBe(false);
    expect(favorites.hasTrack(1)).toBe(false);
  });

  it("clears everything on reset", async () => {
    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [1], albums: [2], artists: [3] });
    const favorites = createFavorites(fakeToasts());
    await favorites.hydrate();
    vi.mocked(api.setFavoriteTrack).mockRejectedValueOnce(new Error("boom"));
    await favorites.toggleTrack(9);

    favorites.reset();

    expect(favorites.hydrated).toBe(false);
    expect(favorites.hasTrack(1)).toBe(false);
    expect(favorites.hasAlbum(2)).toBe(false);
    expect(favorites.hasArtist(3)).toBe(false);
    expect(favorites.trackRevision).toBe(0);
    expect(favorites.pendingRemovalsToDrop(new Set([1, 2, 3]))).toEqual([]);
  });

  it("does not report a re-favorited track for removal", async () => {
    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [1, 2], albums: [], artists: [] });
    const favorites = createFavorites(fakeToasts());
    await favorites.hydrate();

    expect(await favorites.toggleTrack(1)).toBe(false);
    expect(favorites.pendingRemovalsToDrop(new Set([1, 2]))).toEqual([1]);

    expect(await favorites.toggleTrack(1)).toBe(true);
    expect(favorites.pendingRemovalsToDrop(new Set([1, 2]))).toEqual([]);
  });

  it("does not resurrect a track unfavorited while a hydrate is in flight", async () => {
    vi.mocked(api.getFavoriteIds).mockRejectedValueOnce(new Error("offline"));
    const favorites = createFavorites(fakeToasts());
    await favorites.hydrate();
    expect(await favorites.toggleTrack(7)).toBe(true);

    let resolveIds: ((ids: FavoriteIds) => void) | null = null;
    vi.mocked(api.getFavoriteIds).mockImplementation(
      () =>
        new Promise<FavoriteIds>((resolve) => {
          resolveIds = resolve;
        }),
    );
    const hydrating = favorites.hydrate();
    expect(await favorites.toggleTrack(7)).toBe(false);
    resolveIds!({ tracks: [7], albums: [], artists: [] });
    await hydrating;

    expect(favorites.hasTrack(7)).toBe(false);
  });

  it("hydrates to empty sets without error", async () => {
    const favorites = createFavorites(fakeToasts());

    await favorites.hydrate();

    expect(favorites.hydrated).toBe(true);
    expect(favorites.hasTrack(1)).toBe(false);
    expect(favorites.hasAlbum(1)).toBe(false);
    expect(favorites.hasArtist(1)).toBe(false);
  });
});
