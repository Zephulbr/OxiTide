import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Artist, HistoryEntry, Track } from "./api";
import {
  buildSuggestions,
  clearSuggestCache,
  groupRecentAlbums,
  resolveGroups,
  type SuggestApi,
} from "./homeSuggest";

const entry = (track_id: number, album: string, artist: string): HistoryEntry => ({
  track_id, title: `t${track_id}`, artist, album,
  duration_secs: 100, cover: `c-${album}`, started_at: 0, listened_ms: 0, completed: true,
});

const track = (id: number, albumId: number, artistId: number, artistName: string): Track => ({
  id, title: `t${id}`, duration: 100, trackNumber: 1, volumeNumber: 1, explicit: false,
  artists: [{ id: artistId, name: artistName }],
  album: { id: albumId, title: `al${albumId}`, cover: `cover-${albumId}` },
});

const artist = (id: number): Artist => ({ id, name: `artist ${id}` });

beforeEach(() => clearSuggestCache());

describe("groupRecentAlbums", () => {
  it("groups by album + artist, newest first, counting plays and skipping blank albums", () => {
    const groups = groupRecentAlbums([
      entry(1, "A", "x"),
      entry(2, "B", "y"),
      entry(3, "A", "x"),
      entry(4, "", "z"),
      entry(5, "A", "other"),
    ]);
    expect(groups.map((g) => [g.album, g.artist, g.plays])).toEqual([
      ["A", "x", 2],
      ["B", "y", 1],
      ["A", "other", 1],
    ]);
    expect(groups[0].sample.track_id).toBe(1);
  });

  it("honours the cap", () => {
    const entries = [1, 2, 3, 4].map((n) => entry(n, `al${n}`, "x"));
    expect(groupRecentAlbums(entries, 2)).toHaveLength(2);
  });
});

describe("resolveGroups", () => {
  it("resolves ids, prefers the album cover, and drops failed lookups", async () => {
    const getTrack = vi.fn(async (id: number) => {
      if (id === 2) throw new Error("nope");
      return track(id, 100 + id, 7, "Seven");
    });
    const groups = groupRecentAlbums([entry(1, "A", "x"), entry(2, "B", "y")]);
    const out = await resolveGroups(groups, getTrack);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ albumId: 101, artistId: 7, artistName: "Seven", cover: "cover-101" });
  });

  it("merges groups that resolve to the same album (multi-artist soundtracks)", async () => {
    // Same album name, tracks credited to different artists: two groups, one album.
    const getTrack = vi.fn(async (id: number) => track(id, 500, id === 1 ? 7 : 8, "Ost"));
    const groups = groupRecentAlbums([entry(1, "OST", "x"), entry(2, "OST", "y"), entry(3, "OST", "y")]);
    expect(groups).toHaveLength(2);
    const out = await resolveGroups(groups, getTrack);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ albumId: 500, artistId: 7, plays: 3 });
  });

  it("caches resolved tracks across calls", async () => {
    const getTrack = vi.fn(async (id: number) => track(id, 1, 1, "A"));
    const groups = groupRecentAlbums([entry(1, "A", "x")]);
    await resolveGroups(groups, getTrack);
    await resolveGroups(groups, getTrack);
    expect(getTrack).toHaveBeenCalledTimes(1);
  });
});

describe("buildSuggestions", () => {
  const history = [
    entry(1, "A", "x"),
    entry(2, "B", "x"),
    entry(3, "C", "y"),
  ];
  const getTrack = async (id: number) =>
    id === 3 ? track(3, 30, 2, "Y") : track(id, id * 10, 1, "X");

  it("ranks artists by recent plays and seeds radios and similar artists from them", async () => {
    const getSimilarArtists = vi.fn(async () => [artist(1), artist(5), artist(6)]);
    const api: SuggestApi = { getTrack, getSimilarArtists };
    const out = await buildSuggestions(history, api);

    expect(out.albums.map((a) => a.albumId)).toEqual([10, 20, 30]);
    expect(out.radios.map((r) => r.key)).toEqual(["artist-1", "artist-2", "track-1"]);
    expect(getSimilarArtists).toHaveBeenCalledWith(1);
    expect(out.similarTo).toEqual({ id: 1, name: "X" });
    // The seed artist itself is filtered out of its own suggestions.
    expect(out.similar.map((a) => a.id)).toEqual([5, 6]);
  });

  it("survives a failing similar-artists call", async () => {
    const api: SuggestApi = {
      getTrack,
      getSimilarArtists: async () => {
        throw new Error("boom");
      },
    };
    const out = await buildSuggestions(history, api);
    expect(out.similar).toEqual([]);
    expect(out.similarTo).toBeNull();
    expect(out.albums).toHaveLength(3);
  });

  it("returns nothing for an empty history", async () => {
    const api: SuggestApi = { getTrack, getSimilarArtists: async () => [] };
    expect(await buildSuggestions([], api)).toEqual({
      albums: [], radios: [], similarTo: null, similar: [],
    });
  });
});
