import { describe, expect, it } from "vitest";
import { makeSyntheticAlbum, makeSyntheticTracks } from "./perfFixture";

describe("makeSyntheticTracks", () => {
  it("is deterministic and unique", () => {
    const a = makeSyntheticTracks(10_000);
    const b = makeSyntheticTracks(10_000);
    expect(a).toHaveLength(10_000);
    expect(new Set(a.map((t) => t.id)).size).toBe(10_000);
    expect(a[123].title).toBe(b[123].title);
    expect(a[123].artists[0].name).toBe(b[123].artists[0].name);
  });

  it("spreads titles so filter queries narrow the list", () => {
    const tracks = makeSyntheticTracks(10_000);
    const matches = tracks.filter((t) => t.title.toLowerCase().includes("the ")).length;
    expect(matches).toBeGreaterThan(0);
    expect(matches).toBeLessThan(tracks.length);
  });

  it("builds a deterministic 500-track album", () => {
    const { album, tracks } = makeSyntheticAlbum(500);
    expect(album.id).toBe(-1);
    expect(album.title).toContain("500");
    expect(album.numberOfTracks).toBe(500);
    expect(tracks).toHaveLength(500);
    expect(tracks[0].album?.id).toBe(album.id);
    const again = makeSyntheticAlbum(500);
    expect(again.tracks[42].title).toBe(tracks[42].title);
  });
});
