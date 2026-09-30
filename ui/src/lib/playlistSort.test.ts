import { describe, expect, it } from "vitest";
import type { Playlist } from "./api";
import { parseTidalTime, sortPlaylistsByRecency } from "./playlistSort";

function playlist(uuid: string, fields: Partial<Playlist> = {}): Playlist {
  return {
    uuid,
    title: uuid,
    description: null,
    numberOfTracks: 0,
    duration: 0,
    image: null,
    squareImage: null,
    owned: true,
    lastItemAddedAt: null,
    lastUpdated: null,
    ...fields,
  };
}

describe("parseTidalTime", () => {
  it("parses TIDAL's bare +0000 offset like a Z timestamp", () => {
    expect(parseTidalTime("2026-09-29T20:19:47.004+0000")).toBe(
      Date.parse("2026-09-29T20:19:47.004Z"),
    );
  });

  it("returns 0 for null, undefined, and garbage", () => {
    expect(parseTidalTime(null)).toBe(0);
    expect(parseTidalTime(undefined)).toBe(0);
    expect(parseTidalTime("not a date")).toBe(0);
  });
});

describe("sortPlaylistsByRecency", () => {
  it("orders the most recently added-to playlist first", () => {
    const items = [
      playlist("old", { lastItemAddedAt: "2026-01-01T00:00:00.000+0000" }),
      playlist("new", { lastItemAddedAt: "2026-09-29T20:19:47.004+0000" }),
      playlist("mid", { lastItemAddedAt: "2026-05-05T05:05:05.000+0000" }),
    ];
    expect(sortPlaylistsByRecency(items).map((p) => p.uuid)).toEqual(["new", "mid", "old"]);
  });

  it("ranks by lastItemAddedAt even when another playlist was edited later", () => {
    const items = [
      playlist("edited-later", {
        lastItemAddedAt: "2026-09-20T00:00:00.000+0000",
        lastUpdated: "2026-09-30T00:00:00.000+0000",
      }),
      playlist("added-later", {
        lastItemAddedAt: "2026-09-29T00:00:00.000+0000",
        lastUpdated: "2026-09-29T00:00:00.000+0000",
      }),
    ];
    expect(sortPlaylistsByRecency(items).map((p) => p.uuid)).toEqual([
      "added-later",
      "edited-later",
    ]);
  });

  it("falls back to lastUpdated when lastItemAddedAt is null", () => {
    const items = [
      playlist("never-added", { lastUpdated: "2026-09-30T00:00:00.000+0000" }),
      playlist("added", { lastItemAddedAt: "2026-09-29T00:00:00.000+0000" }),
    ];
    expect(sortPlaylistsByRecency(items).map((p) => p.uuid)).toEqual(["never-added", "added"]);
  });

  it("keeps API order for playlists without timestamps", () => {
    const items = [playlist("first"), playlist("second"), playlist("third")];
    expect(sortPlaylistsByRecency(items).map((p) => p.uuid)).toEqual([
      "first",
      "second",
      "third",
    ]);
  });

  it("sorts unparseable timestamps last", () => {
    const items = [
      playlist("broken", { lastItemAddedAt: "whenever" }),
      playlist("valid", { lastItemAddedAt: "2026-01-01T00:00:00.000+0000" }),
    ];
    expect(sortPlaylistsByRecency(items).map((p) => p.uuid)).toEqual(["valid", "broken"]);
  });

  it("does not mutate the input array", () => {
    const items = [playlist("a"), playlist("b")];
    const sorted = sortPlaylistsByRecency(items);
    expect(items.map((p) => p.uuid)).toEqual(["a", "b"]);
    expect(sorted).not.toBe(items);
  });
});
