import { describe, expect, it } from "vitest";
import { hereTargets, menuArtists } from "./nameLinks";

describe("hereTargets", () => {
  it("reads the album and artist pages, including the artist's full lists", () => {
    expect(hereTargets({ name: "album", id: 7 })).toEqual({ albumId: 7, artistId: null });
    expect(hereTargets({ name: "artist", id: 3 })).toEqual({ albumId: null, artistId: 3 });
    expect(hereTargets({ name: "artist-all", id: 3, tab: "tracks" })).toEqual({ albumId: null, artistId: 3 });
    expect(hereTargets({ name: "playlist", id: "p" })).toEqual({ albumId: null, artistId: null });
  });
});

describe("menuArtists", () => {
  const a = (id: number) => ({ id, name: `A${id}` });

  it("drops the current artist and duplicates, keeping at most three", () => {
    const here = { albumId: null, artistId: 2 };
    expect(menuArtists([a(1), a(2), a(1), a(3), a(4), a(5)], here).map((x) => x.id)).toEqual([1, 3, 4]);
  });

  it("keeps everyone off an artist page", () => {
    expect(menuArtists([a(1), a(2)], null).map((x) => x.id)).toEqual([1, 2]);
  });
});
