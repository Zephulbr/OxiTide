import { describe, expect, it } from "vitest";
import type { Album } from "./api";
import { sortAlbums } from "./albumSort";

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

const ids = (list: Album[]) => list.map((a) => a.id);

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
