import { describe, expect, it } from "vitest";
import type { Artist } from "./api";
import { groupArtists, visibleArtists } from "./artistSort";

function artist(id: number, name: string): Artist {
  return { id, name, picture: null };
}

describe("visibleArtists", () => {
  const artists = [artist(1, "Radiohead"), artist(2, "arctic monkeys"), artist(3, "Björk")];

  it("returns every artist in name order by default", () => {
    expect(visibleArtists(artists, "", "name").map((a) => a.name)).toEqual([
      "arctic monkeys",
      "Björk",
      "Radiohead",
    ]);
  });

  it("sorts descending when asked", () => {
    expect(visibleArtists(artists, "", "name-desc").map((a) => a.name)).toEqual([
      "Radiohead",
      "Björk",
      "arctic monkeys",
    ]);
  });

  it("filters case-insensitively on a trimmed query", () => {
    expect(visibleArtists(artists, "  MONKEY ", "name").map((a) => a.name)).toEqual([
      "arctic monkeys",
    ]);
  });

  it("returns nothing when the query matches nothing", () => {
    expect(visibleArtists(artists, "zzz", "name")).toEqual([]);
  });

  it("does not mutate the input array", () => {
    const input = [...artists];
    visibleArtists(input, "", "name-desc");
    expect(input).toEqual(artists);
  });

  it("keeps the API order in date-added mode", () => {
    expect(visibleArtists(artists, "", "date").map((a) => a.name)).toEqual([
      "Radiohead",
      "arctic monkeys",
      "Björk",
    ]);
    expect(visibleArtists(artists, "bj", "date").map((a) => a.name)).toEqual(["Björk"]);
  });
});

describe("groupArtists", () => {
  it("groups consecutive artists by folded initial", () => {
    const list = [
      artist(1, "AC/DC"),
      artist(2, "Ado"),
      artist(3, "Aerosmith"),
      artist(4, "Björk"),
      artist(5, "bad company"),
    ];
    expect(groupArtists(list).map((g) => [g.letter, g.artists.length])).toEqual([
      ["A", 3],
      ["B", 2],
    ]);
  });

  it("folds diacritics and buckets non-letters under #", () => {
    expect(groupArtists([artist(1, "Ólafur"), artist(2, "3 Doors Down")]).map((g) => g.letter)).toEqual([
      "O",
      "#",
    ]);
  });

  it("preserves descending order for Z-A groups", () => {
    const list = [artist(1, "Zoo"), artist(2, "Yes"), artist(3, "XTC")];
    expect(groupArtists(list).map((g) => g.letter)).toEqual(["Z", "Y", "X"]);
  });
});
