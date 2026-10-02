import { describe, expect, it } from "vitest";
import type { CollectionEntry, Playlist } from "./api";
import {
  filterCollectionEntries,
  parseTidalTime,
  sortCollectionEntries,
  sortPlaylistsByRecency,
  type CollectionSortOptions,
} from "./playlistSort";

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

function folder(id: string): CollectionEntry {
  return { kind: "folder", id, name: id, totalNumberOfItems: 0 };
}

function entry(uuid: string, fields: Partial<Playlist> = {}): CollectionEntry {
  return { kind: "playlist", ...playlist(uuid, fields) };
}

const ids = (list: readonly CollectionEntry[]) =>
  list.map((e) => (e.kind === "playlist" ? e.uuid : e.id));

const rank = (ranks: Record<string, number>) => (uuid: string) => ranks[uuid] ?? 0;

const options = (over: Partial<CollectionSortOptions> = {}): CollectionSortOptions => ({
  sort: "recent",
  dir: "desc",
  rank: rank({}),
  ...over,
});

describe("sortCollectionEntries", () => {
  it("orders recently played playlists most recent first", () => {
    const entries = [entry("old"), entry("new"), entry("mid")];
    const sorted = sortCollectionEntries(
      entries,
      options({ rank: rank({ old: 100, new: 300, mid: 200 }) }),
    );
    expect(ids(sorted)).toEqual(["new", "mid", "old"]);
  });

  it("keeps never-played playlists below played ones in API order", () => {
    const entries = [entry("never-a"), entry("played"), entry("never-b")];
    const sorted = sortCollectionEntries(entries, options({ rank: rank({ played: 1 }) }));
    expect(ids(sorted)).toEqual(["played", "never-a", "never-b"]);
  });

  it("flips recency with ascending direction", () => {
    const entries = [entry("old"), entry("new"), entry("mid")];
    const sorted = sortCollectionEntries(
      entries,
      options({ dir: "asc", rank: rank({ old: 100, new: 300, mid: 200 }) }),
    );
    expect(ids(sorted)).toEqual(["old", "mid", "new"]);
  });

  it("sorts by created date with missing values last when descending", () => {
    const entries = [
      entry("unknown"),
      entry("old", { created: "2026-01-01T00:00:00.000+0000" }),
      entry("new", { created: "2026-09-29T20:19:47.004+0000" }),
    ];
    const sorted = sortCollectionEntries(entries, options({ sort: "created" }));
    expect(ids(sorted)).toEqual(["new", "old", "unknown"]);
  });

  it("sorts by updated date", () => {
    const entries = [
      entry("old", { lastUpdated: "2025-01-01T00:00:00.000+0000" }),
      entry("new", { lastUpdated: "2026-09-29T20:19:47.004+0000" }),
      entry("mid", { lastUpdated: "2026-05-05T05:05:05.000+0000" }),
    ];
    const sorted = sortCollectionEntries(entries, options({ sort: "updated" }));
    expect(ids(sorted)).toEqual(["new", "mid", "old"]);
  });

  it("sorts alphabetically case-insensitively", () => {
    const entries = [entry("b", { title: "beta" }), entry("a", { title: "Alpha" })];
    const sorted = sortCollectionEntries(entries, options({ sort: "alpha", dir: "asc" }));
    expect(ids(sorted)).toEqual(["a", "b"]);
  });

  it("flips alphabetical with descending direction", () => {
    const entries = [entry("a", { title: "Alpha" }), entry("b", { title: "beta" })];
    const sorted = sortCollectionEntries(entries, options({ sort: "alpha", dir: "desc" }));
    expect(ids(sorted)).toEqual(["b", "a"]);
  });

  it("keeps API order for tied playlists", () => {
    const entries = [entry("first"), entry("second"), entry("third")];
    const sorted = sortCollectionEntries(entries, options({ sort: "updated" }));
    expect(ids(sorted)).toEqual(["first", "second", "third"]);
  });

  it("keeps folders ahead of playlists in their input order", () => {
    const entries = [entry("played"), folder("f2"), entry("quiet"), folder("f1")];
    const sorted = sortCollectionEntries(entries, options({ rank: rank({ played: 1 }) }));
    expect(ids(sorted)).toEqual(["f2", "f1", "played", "quiet"]);
  });

  it("does not mutate the input array", () => {
    const entries = [entry("a"), folder("f"), entry("b")];
    const sorted = sortCollectionEntries(entries, options());
    expect(ids(entries)).toEqual(["a", "f", "b"]);
    expect(sorted).not.toBe(entries);
  });
});

describe("filterCollectionEntries", () => {
  it("returns everything for the all filter", () => {
    const entries = [folder("f"), entry("owned"), entry("followed", { owned: false })];
    expect(ids(filterCollectionEntries(entries, "all"))).toEqual(["f", "owned", "followed"]);
  });

  it("keeps folders and owned playlists for your playlists", () => {
    const entries = [folder("f"), entry("owned"), entry("followed", { owned: false })];
    expect(ids(filterCollectionEntries(entries, "owned"))).toEqual(["f", "owned"]);
  });

  it("keeps only followed playlists and hides folders", () => {
    const entries = [folder("f"), entry("owned"), entry("followed", { owned: false })];
    expect(ids(filterCollectionEntries(entries, "followed"))).toEqual(["followed"]);
  });

  it("does not mutate the input array", () => {
    const entries = [folder("f"), entry("owned"), entry("followed", { owned: false })];
    const filtered = filterCollectionEntries(entries, "owned");
    expect(ids(entries)).toEqual(["f", "owned", "followed"]);
    expect(filtered).not.toBe(entries);
  });
});
