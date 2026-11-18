import { describe, expect, it } from "vitest";
import type { RowTrack, Tile } from "./convert";
import { filterByName, filterRows, filterTiles, matchesThenRest } from "./filter";

function tile(key: string, title: string, subtitle: string): Tile {
  return { key, title, subtitle, image: "", round: false, route: null, track: null };
}

describe("filterTiles", () => {
  const albums = [tile("a", "Power, Corruption & Lies", "New Order · 1983"), tile("b", "Closer", "Joy Division · 1980")];

  it("returns the same list for a blank query", () => {
    expect(filterTiles(albums, " ")).toBe(albums);
  });

  it("matches the title or the subtitle", () => {
    expect(filterTiles(albums, "closer").map((x) => x.key)).toEqual(["b"]);
    expect(filterTiles(albums, "new order").map((x) => x.key)).toEqual(["a"]);
  });

  it("can ignore a subtitle that is only a kind label", () => {
    const artists = [tile("x", "Joy Division", "Artist"), tile("y", "New Order", "Artist")];
    expect(filterTiles(artists, "artist", { subtitle: false })).toEqual([]);
    expect(filterTiles(artists, "order", { subtitle: false }).map((x) => x.key)).toEqual(["y"]);
  });
});

describe("filterByName", () => {
  it("matches the name, case-insensitive", () => {
    const entries = [{ n: "Running" }, { n: "Late night" }, { n: "RUN club" }];
    expect(filterByName(entries, "run", (e) => e.n).map((e) => e.n)).toEqual(["Running", "RUN club"]);
    expect(filterByName(entries, "", (e) => e.n)).toBe(entries);
  });
});

function row(id: number, title: string, artist: string): RowTrack {
  return { id, title, artist, album: "Album", cover: null, duration: 1, explicit: false, artistId: null, albumId: null };
}

const rows = [row(1, "Blue Monday", "New Order"), row(2, "Ceremony", "Joy Division"), row(3, "Regret", "New Order")];

describe("filterRows", () => {
  it("returns the same list for an empty or blank query", () => {
    expect(filterRows(rows, "")).toBe(rows);
    expect(filterRows(rows, "   ")).toBe(rows);
  });

  it("matches title or artist, case-insensitive, trimmed", () => {
    expect(filterRows(rows, " new order ").map((r) => r.id)).toEqual([1, 3]);
    expect(filterRows(rows, "CEREM").map((r) => r.id)).toEqual([2]);
  });

  it("does not match the album", () => {
    expect(filterRows(rows, "album")).toEqual([]);
  });
});

describe("matchesThenRest", () => {
  it("is the list itself when unfiltered", () => {
    expect(matchesThenRest(rows, rows)).toBe(rows);
  });

  it("puts the matches first, then the rest in list order", () => {
    const matches = filterRows(rows, "regret");
    expect(matchesThenRest(matches, rows).map((r) => r.id)).toEqual([3, 1, 2]);
  });

  it("keeps duplicate tracks that are separate rows", () => {
    const dup = [rows[0], row(1, "Blue Monday", "New Order"), rows[1]];
    const matches = filterRows(dup, "ceremony");
    expect(matchesThenRest(matches, dup).map((r) => r.id)).toEqual([2, 1, 1]);
  });
});
