import { describe, expect, it } from "vitest";
import type { Artist, SearchResults } from "./api";
import { kindsWithResults, pickTopHit, playlistCard, shelfOrder, totalOf } from "./searchResults";

const artist = (id: number, name: string): Artist => ({ id, name });

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
