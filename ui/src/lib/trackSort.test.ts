import { describe, expect, it } from "vitest";
import type { Track } from "./api";
import {
  addLevel,
  flipLevel,
  keysFor,
  MAX_LEVELS,
  moveLevel,
  needsData,
  removeLevel,
  setPrimary,
  sortTracks,
  type SortLevel,
} from "./trackSort";

function track(id: number, extra: Partial<Track> = {}): Track {
  return {
    id,
    title: `T${id}`,
    duration: 100,
    artists: [{ id: 1, name: "A" }],
    album: { id: 1, title: "Album" },
    audioQuality: "LOSSLESS",
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
    ...extra,
  };
}

const ids = (list: Track[]) => list.map((t) => t.id);

describe("level operations", () => {
  it("cycles a sole header: default, flipped, off", () => {
    let levels = setPrimary([], "title");
    expect(levels).toEqual([{ key: "title", dir: "asc" }]);
    levels = setPrimary(levels, "title");
    expect(levels).toEqual([{ key: "title", dir: "desc" }]);
    expect(setPrimary(levels, "title")).toEqual([]);
  });

  it("starts date and count keys descending", () => {
    expect(setPrimary([], "added")).toEqual([{ key: "added", dir: "desc" }]);
    expect(setPrimary([], "plays")).toEqual([{ key: "plays", dir: "desc" }]);
  });

  it("replaces every level with a new header, but flips an active one in place", () => {
    const two: SortLevel[] = [
      { key: "artist", dir: "asc" },
      { key: "added", dir: "desc" },
    ];
    expect(setPrimary(two, "title")).toEqual([{ key: "title", dir: "asc" }]);
    expect(setPrimary(two, "added")).toEqual([
      { key: "artist", dir: "asc" },
      { key: "added", dir: "asc" },
    ]);
  });

  it("appends levels and replaces the last past the cap", () => {
    let levels: SortLevel[] = [];
    for (const key of ["artist", "album", "title"] as const) levels = addLevel(levels, key);
    expect(levels.map((l) => l.key)).toEqual(["artist", "album", "title"]);
    levels = addLevel(levels, "plays");
    expect(levels).toHaveLength(MAX_LEVELS);
    expect(levels.map((l) => l.key)).toEqual(["artist", "album", "plays"]);
    expect(addLevel(levels, "artist")[0]).toEqual({ key: "artist", dir: "desc" });
  });

  it("flips, removes and moves levels", () => {
    const levels: SortLevel[] = [
      { key: "artist", dir: "asc" },
      { key: "album", dir: "asc" },
      { key: "title", dir: "asc" },
    ];
    expect(flipLevel(levels, 1)[1].dir).toBe("desc");
    expect(removeLevel(levels, 0).map((l) => l.key)).toEqual(["album", "title"]);
    expect(moveLevel(levels, 2, 0).map((l) => l.key)).toEqual(["title", "artist", "album"]);
    expect(moveLevel(levels, 0, 5).map((l) => l.key)).toEqual(["artist", "album", "title"]);
  });

  it("knows which keys need fetched data", () => {
    expect(needsData([{ key: "title", dir: "asc" }])).toBe(false);
    expect(needsData([{ key: "released", dir: "desc" }])).toBe(true);
  });
});

describe("keysFor", () => {
  it("offers date added only where it exists, and no album keys on an album page", () => {
    expect(keysFor("favorites")).toContain("added");
    expect(keysFor("playlist")).toContain("added");
    expect(keysFor("search")).not.toContain("added");
    expect(keysFor("album")).not.toContain("album");
    expect(keysFor("album")).not.toContain("released");
    expect(keysFor("album")).toContain("plays");
  });
});

describe("sortTracks", () => {
  it("sorts by a secondary key inside primary ties", () => {
    const list = [
      track(1, { artists: [{ id: 2, name: "Beta" }], dateAdded: "2024-01-01T00:00:00.000+0000" }),
      track(2, { artists: [{ id: 1, name: "alpha" }], dateAdded: "2023-01-01T00:00:00.000+0000" }),
      track(3, { artists: [{ id: 2, name: "Beta" }], dateAdded: "2025-01-01T00:00:00.000+0000" }),
    ];
    const sorted = sortTracks(list, [
      { key: "artist", dir: "asc" },
      { key: "added", dir: "desc" },
    ]);
    expect(ids(sorted)).toEqual([2, 3, 1]);
  });

  it("keeps missing values last in both directions", () => {
    const list = [track(1, { popularity: 10 }), track(2), track(3, { popularity: 50 })];
    expect(ids(sortTracks(list, [{ key: "popularity", dir: "desc" }]))).toEqual([3, 1, 2]);
    expect(ids(sortTracks(list, [{ key: "popularity", dir: "asc" }]))).toEqual([1, 3, 2]);
  });

  it("reads plays and fetched release dates from the context; embedded dates win", () => {
    const list = [
      track(1, { album: { id: 10, title: "X" } }),
      track(2, { album: { id: 20, title: "Y", releaseDate: "1992-09-21" } }),
      track(3, { album: { id: 30, title: "Z" } }),
    ];
    const context = {
      plays: new Map([
        [1, 0],
        [2, 5],
      ]),
      releaseDates: new Map<number, string | null>([
        [10, "2001-01-01"],
        [20, "2020-01-01"],
        [30, null],
      ]),
    };
    expect(ids(sortTracks(list, [{ key: "released", dir: "asc" }], context))).toEqual([2, 1, 3]);
    expect(ids(sortTracks(list, [{ key: "plays", dir: "asc" }], context))).toEqual([1, 2, 3]);
  });

  it("keeps album order inside an album level and source order last", () => {
    const list = [
      track(1, { album: { id: 1, title: "A" }, trackNumber: 3 }),
      track(2, { album: { id: 1, title: "A" }, trackNumber: 1 }),
      track(3, { album: { id: 1, title: "A" }, trackNumber: 2, volumeNumber: 2 }),
      track(4, { album: { id: 2, title: "B" }, trackNumber: 1 }),
    ];
    expect(ids(sortTracks(list, [{ key: "album", dir: "asc" }]))).toEqual([2, 1, 3, 4]);
    expect(ids(sortTracks(list, [{ key: "duration", dir: "asc" }]))).toEqual([1, 2, 3, 4]);
  });

  it("ranks quality with hi-res tags on top", () => {
    const list = [
      track(1, { audioQuality: "HIGH" }),
      track(2, { audioQuality: "LOSSLESS", mediaMetadata: { tags: ["HIRES_LOSSLESS"] } }),
      track(3, { audioQuality: "LOSSLESS" }),
    ];
    expect(ids(sortTracks(list, [{ key: "quality", dir: "desc" }]))).toEqual([2, 3, 1]);
  });
});
