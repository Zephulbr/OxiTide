import { describe, expect, it } from "vitest";
import type { LaterEntry } from "./api";
import { initials } from "./collaborators.svelte";
import { laterEntryKey, laterId, laterPageItem } from "./saveForLater";

const track: LaterEntry = {
  kind: "track",
  added_at: "2026-10-15T07:45:45Z",
  item: {
    id: 7,
    title: "i like the way you kiss me",
    duration: 142,
    artists: [{ id: 1, name: "Artemas" }],
    album: { id: 70, title: "A", cover: "cover-70" },
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  },
};
const artist: LaterEntry = { kind: "artist", item: { id: 9, name: "Nine", picture: null, cover_fallback: "fb-9" } };
const playlist: LaterEntry = {
  kind: "playlist",
  item: { uuid: "p-1", title: "Road", numberOfTracks: 1, duration: 0, squareImage: "sq", owned: false },
};

describe("save for later entries", () => {
  it("keys playlists by uuid and the rest by numeric id", () => {
    expect(laterId(track)).toBe("7");
    expect(laterId(playlist)).toBe("p-1");
    expect(laterEntryKey(artist)).toBe("artist:9");
  });

  it("draws a track as a track card with its album cover and artists", () => {
    expect(laterPageItem(track)).toMatchObject({
      id: "7",
      item_type: "TRACK",
      artwork: "cover-70",
      subtitle: "Artemas",
      artist_refs: [{ id: 1, name: "Artemas" }],
    });
  });

  it("falls back to an artist's cover and a playlist's square image", () => {
    expect(laterPageItem(artist)).toMatchObject({ item_type: "ARTIST", artwork: "fb-9", title: "Nine" });
    expect(laterPageItem(playlist)).toMatchObject({ item_type: "PLAYLIST", artwork: "sq", subtitle: "1 track" });
  });

  it("draws a mix as a mix card keyed by its mix id", () => {
    const mix: LaterEntry = {
      kind: "mix",
      item: { id: "0019ab", title: "a tombstone", subTitle: "Kensuke Ushio", image: "https://img/m.jpg" },
    };
    expect(laterEntryKey(mix)).toBe("mix:0019ab");
    expect(laterPageItem(mix)).toEqual({
      id: "0019ab",
      title: "a tombstone",
      subtitle: "Kensuke Ushio",
      item_type: "MIX",
      artwork: "https://img/m.jpg",
    });
  });
});

describe("initials", () => {
  it("takes up to two letters", () => {
    expect(initials("CheeseMan")).toBe("CH");
    expect(initials("ada lovelace")).toBe("AL");
    expect(initials("  ")).toBe("?");
  });
});
