import { describe, expect, it } from "vitest";
import type { Mix, PageItem, Playlist } from "./api";
import {
  classifySection,
  isPlayableTrack,
  playableTrackIds,
  shapeForItemType,
  shortcutEntries,
} from "./media";

function item(item_type: string, id = "1"): PageItem {
  return { id, title: "Item", item_type };
}

describe("shapeForItemType", () => {
  it("maps artists, albums, playlists and mixes", () => {
    expect(shapeForItemType("ARTIST")).toBe("artist");
    expect(shapeForItemType("artist")).toBe("artist");
    expect(shapeForItemType("ALBUM")).toBe("album");
    expect(shapeForItemType("PLAYLIST_LIST")).toBe("playlist");
    expect(shapeForItemType("EDITORIAL")).toBe("playlist");
    expect(shapeForItemType("MIX")).toBe("mix");
    expect(shapeForItemType("MIX_LIST")).toBe("mix");
  });

  it("falls back to plain for tracks, videos and unknown types", () => {
    expect(shapeForItemType("TRACK_LIST")).toBe("plain");
    expect(shapeForItemType("VIDEO")).toBe("plain");
    expect(shapeForItemType("")).toBe("plain");
    expect(shapeForItemType("MIXED_TYPES_LIST")).toBe("plain");
    expect(shapeForItemType("SOMETHING_ELSE")).toBe("plain");
  });
});

describe("classifySection", () => {
  it("returns carousel for an empty section", () => {
    expect(classifySection("ANY", [])).toBe("carousel");
  });

  it("uses list when tracks or videos are the majority", () => {
    expect(
      classifySection("MIXED_TYPES_LIST", [
        item("TRACK_LIST"),
        item("TRACK_LIST"),
        item("ALBUM"),
      ]),
    ).toBe("list");
    expect(classifySection("", [item("VIDEO"), item("VIDEO")])).toBe("list");
  });

  it("uses carousel for mixes, radio and albums", () => {
    expect(classifySection("MIX_LIST", [item("MIX")])).toBe("carousel");
    expect(classifySection("RADIO", [item("ALBUM")])).toBe("carousel");
    expect(classifySection("ALBUM_LIST", [item("ALBUM")])).toBe("carousel");
  });

  it("uses tiles for playlist and editorial sections", () => {
    expect(classifySection("PLAYLIST_LIST", [item("PLAYLIST_LIST"), item("EDITORIAL")])).toBe(
      "tiles",
    );
    expect(classifySection("", [item("EDITORIAL")])).toBe("tiles");
  });

  it("falls back to carousel for unknown kinds", () => {
    expect(classifySection("PAGE_LINKS_CLOUD", [item("SOMETHING")])).toBe("carousel");
  });
});

describe("track playback predicates", () => {
  it("only treats numeric track items as playable", () => {
    expect(isPlayableTrack(item("TRACK_LIST", "55"))).toBe(true);
    expect(isPlayableTrack(item("TRACK", "55"))).toBe(true);
    expect(isPlayableTrack(item("TRACK_LIST", "abc"))).toBe(false);
    expect(isPlayableTrack(item("VIDEO", "55"))).toBe(false);
    expect(isPlayableTrack(item("ALBUM", "55"))).toBe(false);
  });

  it("keeps playable ids in order and skips everything else", () => {
    expect(
      playableTrackIds([
        item("TRACK_LIST", "3"),
        item("ALBUM", "9"),
        item("TRACK", "1"),
        item("TRACK_LIST", "x"),
      ]),
    ).toEqual([3, 1]);
  });
});

function playlist(uuid: string, title: string): Playlist {
  return {
    uuid,
    title,
    numberOfTracks: 0,
    duration: 0,
    owned: true,
    squareImage: `sq-${uuid}`,
    image: `im-${uuid}`,
  };
}

function mix(id: string, title: string): Mix {
  return { id, title, image: `m-${id}` };
}

describe("shortcutEntries", () => {
  it("puts playlists first, then saved mixes", () => {
    const entries = shortcutEntries([playlist("p1", "P1")], [mix("m1", "M1")]);
    expect(entries.map((e) => e.route)).toEqual([
      { name: "playlist", id: "p1" },
      { name: "mix", id: "m1" },
    ]);
    expect(entries[0].item.subtitle).toBe("Playlist");
    expect(entries[1].item.subtitle).toBe("Saved mix");
    expect(entries[0].playlist).toBe(true);
    expect(entries[1].playlist).toBe(false);
  });

  it("caps the strip and tolerates missing sources", () => {
    const playlists = Array.from({ length: 5 }, (_, i) => playlist(`p${i}`, `P${i}`));
    const mixes = Array.from({ length: 3 }, (_, i) => mix(`m${i}`, `M${i}`));
    expect(shortcutEntries(playlists, mixes)).toHaveLength(6);
    expect(shortcutEntries([], [])).toEqual([]);
    expect(shortcutEntries([], mixes).map((e) => e.key)).toEqual(["mix-m0", "mix-m1", "mix-m2"]);
  });
});
