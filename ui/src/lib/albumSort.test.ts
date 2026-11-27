import { describe, expect, it } from "vitest";
import type { Album } from "./api";
import { filterAlbums, sortAlbums } from "./albumSort";

function album(id: number, title: string, artist: string, releaseDate: string | null, dateAdded: string | null): Album {
  return {
    id,
    title,
    artists: [{ id, name: artist }],
    numberOfTracks: 10,
    duration: 0,
    releaseDate,
    dateAdded,
  };
}

const albums = [
  album(1, "beta", "Zed", "2001-05-01", "2024-01-01T00:00:00.000+0000"),
  album(2, "Alpha", "anna", "2019-02-02", null),
  album(3, "Gamma", "Anna", "2010-03-03", "2025-06-01T00:00:00.000+0000"),
];

const ids = (list: readonly Album[]) => list.map((a) => a.id);

describe("filterAlbums", () => {
  const library = [
    album(1, "OK Computer", "Radiohead", null, null),
    album(2, "Lemonade", "Beyoncé", null, null),
    album(3, "Back in Black", "AC/DC", null, null),
    { ...album(4, "Watch the Throne", "JAY-Z", null, null), artists: [{ id: 4, name: "JAY-Z" }, { id: 5, name: "Kanye West" }] },
  ];

  it("matches the title or any credited artist, folding case and accents", () => {
    expect(ids(filterAlbums(library, "computer"))).toEqual([1]);
    expect(ids(filterAlbums(library, "beyonce"))).toEqual([2]);
    expect(ids(filterAlbums(library, "kanye"))).toEqual([4]);
  });

  it("folds punctuation, so a name typed without it still matches", () => {
    expect(ids(filterAlbums(library, "acdc"))).toEqual([3]);
    expect(ids(filterAlbums(library, "ac dc"))).toEqual([3]);
  });

  it("keeps the given order and returns everything for an empty query", () => {
    expect(ids(filterAlbums(library, "  "))).toEqual([1, 2, 3, 4]);
    expect(ids(filterAlbums(library, "on"))).toEqual([2, 4]);
  });

  it("returns nothing when nothing matches", () => {
    expect(filterAlbums(library, "zzz")).toEqual([]);
  });
});

describe("sortAlbums", () => {
  it("puts the newest favorites first, undated ones last", () => {
    expect(ids(sortAlbums(albums, "added"))).toEqual([3, 1, 2]);
  });

  it("puts the newest releases first", () => {
    expect(ids(sortAlbums(albums, "released"))).toEqual([2, 3, 1]);
  });

  it("sorts titles A–Z ignoring case", () => {
    expect(ids(sortAlbums(albums, "title"))).toEqual([2, 1, 3]);
  });

  it("groups by artist ignoring case, newest release first within one", () => {
    expect(ids(sortAlbums(albums, "artist"))).toEqual([2, 3, 1]);
  });

  it("does not reorder its input", () => {
    sortAlbums(albums, "title");
    expect(ids(albums)).toEqual([1, 2, 3]);
  });
});
