import { describe, expect, it, vi } from "vitest";
import type { Artist, SearchResults, Track } from "./api";
import {
  artistRow,
  correctedQuery,
  kindsWithResults,
  pickTopHit,
  playlistCard,
  searchCorrecting,
  shelfOrder,
  totalOf,
} from "./searchResults";

const artist = (id: number, name: string): Artist => ({ id, name });

describe("artistRow", () => {
  it("keeps TIDAL's order but moves imageless artists to the end", () => {
    const row = artistRow([
      { id: 1, name: "a", picture: "p" },
      { id: 2, name: "b" },
      { id: 3, name: "c", cover_fallback: "c" },
      { id: 4, name: "d" },
    ]);
    expect(row.map((a) => a.id)).toEqual([1, 3, 2, 4]);
  });
});

function results(partial: Partial<SearchResults> = {}): SearchResults {
  return {
    tracks: { items: [] },
    albums: { items: [] },
    artists: { items: [] },
    playlists: { items: [] },
    ...partial,
  };
}

describe("pickTopHit", () => {
  it("prefers TIDAL's top hit", () => {
    const top = { kind: "artist" as const, item: artist(2, "Beck") };
    expect(pickTopHit(results({ topHit: top, artists: { items: [artist(1, "beck")] } }), "beck")).toBe(top);
  });

  it("falls back to an artist named like the query, ignoring case and punctuation", () => {
    const r = results({ artists: { items: [artist(1, "Sweet"), artist(2, "AC/DC")] } });
    expect(pickTopHit(r, "ac dc")).toEqual({ kind: "artist", item: artist(2, "AC/DC") });
    expect(pickTopHit(r, "Beyonce")).toBeNull();
  });

  it("ignores accents", () => {
    const r = results({ artists: { items: [artist(3, "Beyoncé")] } });
    expect(pickTopHit(r, "beyonce")?.item).toEqual(artist(3, "Beyoncé"));
  });
});

describe("shelfOrder", () => {
  it("leads with the top hit's type", () => {
    expect(shelfOrder(null)).toEqual(["artists", "albums", "playlists"]);
    expect(shelfOrder({ kind: "album", item: { id: 1, title: "A", artists: [], numberOfTracks: 1, duration: 1 } }))
      .toEqual(["albums", "artists", "playlists"]);
    expect(shelfOrder({ kind: "track", item: { id: 1, title: "T", duration: 1, artists: [], trackNumber: 1, volumeNumber: 1, explicit: false } }))
      .toEqual(["artists", "albums", "playlists"]);
  });
});

describe("result counts", () => {
  it("lists only kinds with results and reports at least what loaded", () => {
    const r = results({ artists: { items: [artist(1, "A")], totalNumberOfItems: 40 }, albums: { items: [] } });
    expect(kindsWithResults(r)).toEqual(["artists"]);
    expect(totalOf(r, "artists")).toBe(40);
    expect(totalOf(results({ artists: { items: [artist(1, "A")] } }), "artists")).toBe(1);
  });
});

describe("playlistCard", () => {
  it("uses the square image and counts tracks", () => {
    const card = playlistCard({
      uuid: "u", title: "Mix", numberOfTracks: 1, duration: 0, squareImage: "sq", image: "im", owned: false,
    });
    expect(card).toMatchObject({ id: "u", item_type: "PLAYLIST", artwork: "sq", subtitle: "1 track" });
  });
});

const song = (id: number): Track => ({
  id,
  title: `Song ${id}`,
  duration: 200,
  artists: [],
  trackNumber: 1,
  volumeNumber: 1,
  explicit: false,
});
const keygen = { kind: "artist" as const, item: artist(7, "Keygen Church") };

describe("correctedQuery", () => {
  it("offers the top hit's name when a typo found nothing but the top hit", () => {
    expect(correctedQuery(results({ topHit: keygen }), "kaygen chuch")).toBe("Keygen Church");
  });

  it("stays put when tracks came back, the name is the query, or there is no top hit", () => {
    expect(correctedQuery(results({ topHit: keygen, tracks: { items: [song(1)] } }), "kaygen chuch")).toBeNull();
    expect(correctedQuery(results({ topHit: keygen }), "keygen church!")).toBeNull();
    expect(correctedQuery(results(), "kaygen chuch")).toBeNull();
  });
});

describe("searchCorrecting", () => {
  const thin = results({ topHit: keygen });
  const full = results({ topHit: keygen, tracks: { items: [song(1)] } });

  it("searches again under the corrected name and answers with it", async () => {
    const search = vi.fn(async (q: string) => (q === "Keygen Church" ? full : thin));
    expect(await searchCorrecting(search, "kaygen chuch")).toEqual({ results: full, query: "Keygen Church" });
    expect(search).toHaveBeenCalledTimes(2);
  });

  it("keeps the first results when the retry finds no tracks or fails", async () => {
    const empty = vi.fn(async () => thin);
    expect(await searchCorrecting(empty, "kaygen chuch")).toEqual({ results: thin, query: "kaygen chuch" });
    const failing = vi.fn(async (q: string) => {
      if (q === "Keygen Church") throw new Error("offline");
      return thin;
    });
    expect(await searchCorrecting(failing, "kaygen chuch")).toEqual({ results: thin, query: "kaygen chuch" });
  });

  it("searches once when the results are not thin", async () => {
    const search = vi.fn(async () => full);
    await searchCorrecting(search, "keygen church");
    expect(search).toHaveBeenCalledTimes(1);
  });
});