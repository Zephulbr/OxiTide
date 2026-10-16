import { describe, expect, it } from "vitest";
import type { PageItem, PageSection } from "./api";
import { isDecadeRow, linkHues, splitQuickLinks, withoutVideoLinks } from "./pageLinks";

const link = (title: string, api_path?: string): PageItem => ({
  id: "",
  title,
  item_type: "PAGE_LINKS",
  ...(api_path ? { api_path } : {}),
});
const section = (title: string, items: PageItem[], kind = "PAGE_LINKS"): PageSection => ({
  title,
  items,
  kind,
});

describe("withoutVideoLinks", () => {
  it("drops video links by title or path, and sections they emptied", () => {
    const out = withoutVideoLinks([
      section("", [link("New"), link("Videos"), link("Clips", "pages/videos_top"), link("HiRes")]),
      section("Only", [link("Videos")]),
      section("Empty", []),
      section("Albums", [{ id: "1", title: "Video Killed", item_type: "ALBUM" }], "ALBUM_LIST"),
    ]);
    expect(out.map((s) => [s.title, s.items.map((i) => i.title)])).toEqual([
      ["", ["New", "HiRes"]],
      ["Empty", []],
      ["Albums", ["Video Killed"]],
    ]);
  });
});

describe("splitQuickLinks", () => {
  it("moves untitled link rows to the front, keeping the rest in order", () => {
    const albums = section("", [{ id: "1", title: "A", item_type: "ALBUM" }], "ALBUM_LIST");
    const { quick, rest } = splitQuickLinks([
      section("Genres", [link("Pop")]),
      albums,
      section("", [link("New"), link("Top")]),
    ]);
    expect(quick.map((s) => s.items[0].title)).toEqual(["New"]);
    expect(rest.map((s) => s.title || s.kind)).toEqual(["Genres", "ALBUM_LIST"]);
  });
});

describe("linkHues", () => {
  it("is stable, case-insensitive and in range", () => {
    const [a, b] = linkHues("Hip-Hop");
    expect(linkHues("hip-hop ")).toEqual([a, b]);
    expect(a).toBeGreaterThanOrEqual(0);
    expect(a).toBeLessThan(360);
    expect(b).toBe((a + 40) % 360);
  });

  it("spreads different names apart", () => {
    const hues = new Set(["Pop", "Jazz", "Latin", "Country", "Classical"].map((t) => linkHues(t)[0]));
    expect(hues.size).toBe(5);
  });
});

describe("isDecadeRow", () => {
  it("accepts rows made only of decades", () => {
    expect(isDecadeRow([link("1960s"), link("70s"), link("2010's")])).toBe(true);
  });

  it("rejects mixed or empty rows", () => {
    expect(isDecadeRow([link("1960s"), link("Jazz")])).toBe(false);
    expect(isDecadeRow([])).toBe(false);
  });
});
