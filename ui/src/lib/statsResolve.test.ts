import { beforeEach, describe, expect, it, vi } from "vitest";
import type { StatsTopEntry, Track } from "./api";
import { clearSuggestCache } from "./homeSuggest";
import { clearStatsResolveCache, resolveAlbum, resolveArtist } from "./statsResolve";

const entry = (name: string, track_id = 7): StatsTopEntry => ({
  name,
  plays: 3,
  listened_ms: 0,
  track_id,
  cover: "c",
});

const track = {
  id: 7,
  title: "T",
  duration: 1,
  artists: [
    { id: 1, name: "Nxcre" },
    { id: 2, name: "The Villains" },
  ],
  album: { id: 40, title: "A" },
  trackNumber: 1,
  volumeNumber: 1,
  explicit: false,
} satisfies Track;

beforeEach(() => {
  clearSuggestCache();
  clearStatsResolveCache();
});

describe("resolveArtist", () => {
  it("prefers the credit whose name matches, with its picture", async () => {
    const api = {
      getTrack: vi.fn(async () => track),
      getArtist: vi.fn(async (id: number) => ({ id, name: "x", picture: `p${id}` })),
    };
    expect(await resolveArtist(entry("The Villains"), api)).toEqual({ id: 2, picture: "p2" });
    // A joined name falls back to the primary artist.
    expect(await resolveArtist(entry("Nxcre, The Villains"), api)).toEqual({ id: 1, picture: "p1" });
    expect(api.getTrack).toHaveBeenCalledTimes(1);
  });

  it("keeps the id when the picture lookup fails", async () => {
    const api = {
      getTrack: vi.fn(async () => track),
      getArtist: vi.fn(async () => {
        throw new Error("offline");
      }),
    };
    expect(await resolveArtist(entry("Nxcre"), api)).toEqual({ id: 1, picture: null });
  });
});

describe("resolveAlbum", () => {
  it("returns the newest play's album id", async () => {
    const api = { getTrack: vi.fn(async () => track), getArtist: vi.fn() };
    expect(await resolveAlbum(entry("A"), api)).toBe(40);
  });
});
