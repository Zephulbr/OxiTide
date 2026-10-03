import { describe, expect, it } from "vitest";
import type { Artist } from "./api";
import { visibleArtists } from "./artistSort";

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
});
