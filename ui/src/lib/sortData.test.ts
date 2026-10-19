import { describe, expect, it, vi } from "vitest";
import type { Track } from "./api";
import { createSortData } from "./sortData.svelte";

function track(id: number, albumId: number, releaseDate?: string): Track {
  return {
    id,
    title: `T${id}`,
    duration: 1,
    artists: [],
    album: { id: albumId, title: "A", releaseDate },
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  };
}

describe("sortData", () => {
  it("asks only for albums without an embedded or known date, once", async () => {
    const getAlbumReleaseDates = vi.fn(async (ids: number[]) =>
      ids.map((id) => ({ album_id: id, release_date: id === 2 ? null : "2001-01-01" })),
    );
    const data = createSortData({ getPlayCounts: vi.fn(), getAlbumReleaseDates });
    const tracks = [track(1, 1), track(2, 1), track(3, 2), track(4, 3, "1999-01-01")];

    await data.ensure(["released"], tracks);
    expect(getAlbumReleaseDates).toHaveBeenCalledWith([1, 2]);
    expect(data.context.releaseDates.get(1)).toBe("2001-01-01");
    expect(data.context.releaseDates.get(2)).toBeNull();

    await data.ensure(["released"], tracks, true);
    expect(getAlbumReleaseDates).toHaveBeenCalledTimes(1);
  });

  it("skips failed albums until a fresh ensure retries them", async () => {
    const getAlbumReleaseDates = vi.fn(async () => {
      throw new Error("offline");
    });
    const data = createSortData({ getPlayCounts: vi.fn(), getAlbumReleaseDates });
    await data.ensure(["released"], [track(1, 1)]);
    await data.ensure(["released"], [track(1, 1)]);
    expect(getAlbumReleaseDates).toHaveBeenCalledTimes(1);
    await data.ensure(["released"], [track(1, 1)], true);
    expect(getAlbumReleaseDates).toHaveBeenCalledTimes(2);
  });

  it("loads play counts, and re-reads them when fresh", async () => {
    let plays = 1;
    const getPlayCounts = vi.fn(async (ids: number[]) =>
      ids.map((id) => ({ track_id: id, plays })),
    );
    const data = createSortData({ getPlayCounts, getAlbumReleaseDates: vi.fn() });
    await data.ensure(["plays"], [track(1, 1), track(1, 1)]);
    expect(getPlayCounts).toHaveBeenCalledWith([1]);
    plays = 4;
    await data.ensure(["plays"], [track(1, 1)]);
    expect(data.context.plays.get(1)).toBe(1);
    await data.ensure(["plays"], [track(1, 1)], true);
    expect(data.context.plays.get(1)).toBe(4);
  });
});
