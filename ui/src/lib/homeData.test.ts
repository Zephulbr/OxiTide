import { describe, expect, it } from "vitest";
import type { FeedActivity, PageSection, Stats } from "./api";
import {
  editorialSections,
  feedAlbumItems,
  greeting,
  lastDays,
  madeForYou,
  unseenCount,
} from "./homeData";

const section = (title: string, types: string[]): PageSection => ({
  title,
  kind: "X",
  items: types.map((item_type, i) => ({ id: `${title}-${i}`, title: `${title} ${i}`, item_type })),
});

describe("greeting", () => {
  it("follows the local hour", () => {
    expect(greeting(5)).toBe("Good morning");
    expect(greeting(11)).toBe("Good morning");
    expect(greeting(12)).toBe("Good afternoon");
    expect(greeting(18)).toBe("Good evening");
    expect(greeting(23)).toBe("Good night");
    expect(greeting(3)).toBe("Good night");
  });
});

describe("madeForYou", () => {
  it("picks mix cards across rows", () => {
    const out = madeForYou([
      section("a", ["ALBUM", "MIX_LIST"]),
      section("b", ["MIX_LIST", "ARTIST"]),
    ]);
    expect(out.map((i) => i.id)).toEqual(["a-1", "b-0"]);
  });

  it("falls back to the first non-empty row when there are no mixes", () => {
    const out = madeForYou([section("empty", []), section("b", ["ALBUM", "ALBUM"])]);
    expect(out.map((i) => i.id)).toEqual(["b-0", "b-1"]);
  });

  it("drops duplicate ids", () => {
    const dup = section("a", ["MIX_LIST"]);
    expect(madeForYou([dup, dup])).toHaveLength(1);
  });
});

describe("feed helpers", () => {
  const activity = (id: number, seen: boolean): FeedActivity => ({
    id, title: `r${id}`, subtitle: "Album · X", artwork: "u", explicit: false,
    hires: false, occurredAt: "2026-10-01T00:00:00Z", seen,
  });

  it("maps releases to album cards", () => {
    const [first] = feedAlbumItems([activity(7, false)]);
    expect(first).toMatchObject({ id: "7", item_type: "ALBUM", title: "r7", artwork: "u" });
  });

  it("counts unseen releases", () => {
    expect(unseenCount([activity(1, false), activity(2, true), activity(3, false)])).toBe(2);
  });
});

describe("editorialSections", () => {
  it("skips empty rows and caps the count", () => {
    const rows = [section("a", ["ALBUM"]), section("b", []), section("c", ["ALBUM"])];
    expect(editorialSections(rows, 1).map((s) => s.title)).toEqual(["a"]);
    expect(editorialSections(rows).map((s) => s.title)).toEqual(["a", "c"]);
  });
});

describe("lastDays", () => {
  it("returns the trailing days", () => {
    const stats = {
      daily: [1, 2, 3, 4].map((plays) => ({ date: `2026-10-0${plays}`, plays, listened_ms: 0 })),
    } as Stats;
    expect(lastDays(stats, 2).map((d) => d.plays)).toEqual([3, 4]);
  });
});
