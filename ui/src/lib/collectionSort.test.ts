import { describe, expect, it } from "vitest";
import type { CollectionEntry } from "./api";
import { sortEntries } from "./collectionSort";

function folder(id: string, name: string): CollectionEntry {
  return { kind: "folder", id, name, totalNumberOfItems: 1 };
}

function playlist(uuid: string, title: string): CollectionEntry {
  return {
    kind: "playlist",
    uuid,
    title,
    description: null,
    numberOfTracks: 0,
    duration: 0,
    image: null,
    owned: true,
  };
}

describe("sortEntries", () => {
  it("keeps API order for date mode", () => {
    const entries = [playlist("b", "Beta"), folder("f", "Zeta")];
    expect(sortEntries(entries, "date")).toEqual(entries);
  });

  it("sorts folders first then playlists by name", () => {
    const entries = [
      playlist("z", "zebra"),
      folder("a", "alpha"),
      playlist("a", "Apple"),
      folder("z", "Zulu"),
    ];
    expect(sortEntries(entries, "name").map((e) => (e.kind === "folder" ? e.name : e.title))).toEqual([
      "alpha",
      "Zulu",
      "Apple",
      "zebra",
    ]);
  });
});
