import { describe, expect, it } from "vitest";
import type { Album, Artist, PageItem, PageSection } from "./api";
import { albumCard, artistCard, isPlaylistPageItem, playlistsFromSections } from "./artistPage";

function album(partial: Partial<Album>): Album {
  return {
    id: 1,
    title: "Album",
    artists: [],
    cover: "cover-1",
    numberOfTracks: 10,
    duration: 100,
    releaseDate: null,
    ...partial,
  };
}

function item(item_type: string, id: string): PageItem {
  return { id, title: "Item", item_type };
}

describe("albumCard", () => {
  it("uses the release year as subtitle", () => {
    const card = albumCard(album({ id: 7, title: "Neon Golden", releaseDate: "2002-01-14" }));
    expect(card).toEqual({
      id: "7",
      title: "Neon Golden",
      subtitle: "2002",
      item_type: "ALBUM",
      artwork: "cover-1",
    });
  });

  it("drops a missing or malformed release date", () => {
    expect(albumCard(album({ releaseDate: null })).subtitle).toBeNull();
    expect(albumCard(album({ releaseDate: "soon" })).subtitle).toBeNull();
  });
});

describe("artistCard", () => {
  it("maps an artist onto a circular page item", () => {
    const artist: Artist = { id: 42, name: "Ado", picture: "pic-1" };
    expect(artistCard(artist)).toEqual({
      id: "42",
      title: "Ado",
      subtitle: null,
      item_type: "ARTIST",
      artwork: "pic-1",
    });
  });
});

describe("isPlaylistPageItem", () => {
  it("accepts playlist-ish types including the raw USER subtype", () => {
    expect(isPlaylistPageItem(item("PLAYLIST", "u1"))).toBe(true);
    expect(isPlaylistPageItem(item("PLAYLIST_LIST", "u1"))).toBe(true);
    expect(isPlaylistPageItem(item("EDITORIAL", "u1"))).toBe(true);
    expect(isPlaylistPageItem(item("USER", "u1"))).toBe(true);
    expect(isPlaylistPageItem(item("ALBUM", "1"))).toBe(false);
    expect(isPlaylistPageItem(item("TRACK", "1"))).toBe(false);
  });
});

describe("playlistsFromSections", () => {
  it("returns the section whose items are all playlists", () => {
    const sections: PageSection[] = [
      { title: "Top tracks", kind: "TRACK_LIST", items: [item("TRACK", "1")] },
      { title: "Playlists", kind: "MIXED_TYPES_LIST", items: [item("PLAYLIST", "u1")] },
      { title: "Albums", kind: "ALBUM_LIST", items: [item("ALBUM", "2")] },
    ];
    expect(playlistsFromSections(sections).map((i) => i.id)).toEqual(["u1"]);
  });

  it("returns empty when no section is playlist-only", () => {
    const sections: PageSection[] = [
      { title: "Mixed", kind: "MIXED_TYPES_LIST", items: [item("PLAYLIST", "u1"), item("ALBUM", "2")] },
    ];
    expect(playlistsFromSections(sections)).toEqual([]);
    expect(playlistsFromSections([])).toEqual([]);
  });
});
