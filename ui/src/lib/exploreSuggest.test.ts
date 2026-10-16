import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Artist, HistoryEntry, Track } from "./api";
import { buildNewToYou, topArtistNames } from "./exploreSuggest";
import { clearSuggestCache, type SuggestApi } from "./homeSuggest";

const entry = (
  track_id: number,
  album: string,
  artist: string,
  refs?: { id: number; name: string }[],
): HistoryEntry => ({
  track_id, title: `t${track_id}`, artist, album,
  duration_secs: 100, cover: null, started_at: 0, listened_ms: 0, completed: true,
  ...(refs ? { artist_refs: refs } : {}),
});

const track = (id: number, albumId: number, artistId: number, artistName: string): Track => ({
  id, title: `t${id}`, duration: 100, trackNumber: 1, volumeNumber: 1, explicit: false,
  artists: [{ id: artistId, name: artistName }],
  album: { id: albumId, title: `al${albumId}`, cover: null },
});

const artist = (id: number, name = `artist ${id}`): Artist => ({ id, name });

beforeEach(() => clearSuggestCache());

describe("topArtistNames", () => {
  it("ranks primary artists by plays, preferring refs over the joined string", () => {
    const names = topArtistNames([
      entry(1, "A", "Solo"),
      entry(2, "B", "Duo One, Duo Two", [{ id: 5, name: "Duo One" }, { id: 6, name: "Duo Two" }]),
      entry(3, "C", "Duo One", [{ id: 5, name: "Duo One" }]),
      entry(4, "D", "solo"),
      entry(5, "E", "Rare"),
    ]);
    expect(names).toEqual(["Solo", "Duo One", "Rare"]);
  });

  it("caps the list", () => {
    expect(topArtistNames([entry(1, "A", "a"), entry(2, "B", "b")], 1)).toEqual(["a"]);
  });
});

describe("buildNewToYou", () => {
  const history = [
    entry(1, "A", "Seed One"),
    entry(2, "A", "Seed One"),
    entry(3, "B", "Seed Two"),
    entry(4, "C", "Heard Elsewhere", [{ id: 40, name: "Heard Elsewhere" }]),
  ];
  const tracks: Record<number, Track> = {
    1: track(1, 100, 10, "Seed One"),
    2: track(2, 100, 10, "Seed One"),
    3: track(3, 200, 20, "Seed Two"),
    4: track(4, 300, 40, "Heard Elsewhere"),
  };
  const getTrack = (id: number) => Promise.resolve(tracks[id]);

  it("interleaves seeds and drops heard, favourite and duplicate artists", async () => {
    const api: SuggestApi = {
      getTrack,
      getSimilarArtists: vi.fn(async (id: number) =>
        id === 10
          ? [artist(20), artist(51), artist(52), artist(99)]
          : id === 20
            ? [artist(61), artist(51), artist(40), artist(70, "heard elsewhere")]
            : [],
      ),
    };
    const out = await buildNewToYou(history, api, (id) => id === 99);
    expect(out.map((n) => [n.artist.id, n.seed])).toEqual([
      [61, "Seed Two"],
      [51, "Seed One"],
      [52, "Seed One"],
    ]);
  });

  it("keeps the other seeds when one lookup fails, and caps", async () => {
    const api: SuggestApi = {
      getTrack,
      getSimilarArtists: async (id: number) => {
        if (id === 10) throw new Error("offline");
        return [artist(61), artist(62), artist(63)];
      },
    };
    const out = await buildNewToYou(history, api, undefined, { cap: 2 });
    expect(out.map((n) => n.artist.id)).toEqual([61, 62]);
  });

  it("is empty without history", async () => {
    const api: SuggestApi = { getTrack, getSimilarArtists: vi.fn() };
    expect(await buildNewToYou([], api)).toEqual([]);
    expect(api.getSimilarArtists).not.toHaveBeenCalled();
  });
});
