import { describe, expect, it } from "vitest";
import {
  COLLECTION_KEYS,
  LAYOUT_KEY,
  createCollectionLayout,
  fromRows,
  hiddenKeys,
  moveRow,
  normalizeLayout,
  toRows,
  visibleKeys,
} from "./collectionLayout.svelte";

type LayoutStorage = Pick<Storage, "getItem" | "setItem">;

function fakeStorage(seed?: string): LayoutStorage & { saved(): unknown } {
  const map = new Map<string, string>();
  if (seed !== undefined) map.set(LAYOUT_KEY, seed);
  return {
    getItem: (key) => map.get(key) ?? null,
    setItem: (key, value) => void map.set(key, value),
    saved: () => JSON.parse(map.get(LAYOUT_KEY) ?? "null"),
  };
}

describe("collection layout helpers", () => {
  it("defaults to today's split: four shown, three under Show more", () => {
    const prefs = normalizeLayout(undefined);
    expect(visibleKeys(prefs)).toEqual(["playlists", "albums", "tracks", "later"]);
    expect(hiddenKeys(prefs)).toEqual(["mixes", "profiles", "purchases"]);
    expect(prefs.open).toBe(false);
  });

  it("drops unknown and repeated keys and appends missing ones below the line", () => {
    const prefs = normalizeLayout({
      order: ["tracks", "bogus", "tracks", "albums"],
      split: 5,
      open: true,
    });
    expect(prefs.order.slice(0, 2)).toEqual(["tracks", "albums"]);
    expect(new Set(prefs.order)).toEqual(new Set(COLLECTION_KEYS));
    expect(prefs.order).toHaveLength(COLLECTION_KEYS.length);
    // The split is clamped to the saved keys, so the new ones start hidden.
    expect(prefs.split).toBe(2);
    expect(prefs.open).toBe(true);
  });

  it("accepts all-hidden and all-shown splits", () => {
    expect(normalizeLayout({ order: [...COLLECTION_KEYS], split: 0 }).split).toBe(0);
    expect(normalizeLayout({ order: [...COLLECTION_KEYS], split: 7 }).split).toBe(7);
    expect(normalizeLayout({ order: [...COLLECTION_KEYS], split: -3 }).split).toBe(0);
    expect(normalizeLayout({ order: [...COLLECTION_KEYS], split: 1.5 }).split).toBe(4);
  });

  it("round-trips rows with the line at the split", () => {
    const rows = toRows({ order: ["albums", "playlists"] as never, split: 1 });
    expect(rows).toEqual(["albums", "line", "playlists"]);
    expect(fromRows(rows)).toEqual({ order: ["albums", "playlists"], split: 1 });
  });

  it("moves a row, the line included", () => {
    expect(moveRow(["a", "b", "line", "c"] as never, 0, 3)).toEqual(["b", "line", "c", "a"]);
    expect(moveRow(["a", "b", "line", "c"] as never, 2, 0)).toEqual(["line", "a", "b", "c"]);
  });
});

describe("collection layout store", () => {
  it("moves an entry below the line and saves it", () => {
    const storage = fakeStorage();
    const layout = createCollectionLayout(storage);
    // Rows: playlists albums tracks later | line | mixes profiles purchases
    layout.move(0, 4);
    expect(visibleKeys(layout.prefs)).toEqual(["albums", "tracks", "later"]);
    expect(hiddenKeys(layout.prefs)).toEqual(["playlists", "mixes", "profiles", "purchases"]);
    expect(storage.saved()).toMatchObject({ split: 3 });
  });

  it("moves the line to hide or show everything", () => {
    const layout = createCollectionLayout(fakeStorage());
    layout.move(4, 0);
    expect(visibleKeys(layout.prefs)).toEqual([]);
    layout.move(0, 7);
    expect(hiddenKeys(layout.prefs)).toEqual([]);
  });

  it("restores a saved layout, and survives corrupt data", () => {
    const saved = JSON.stringify({ order: [...COLLECTION_KEYS].reverse(), split: 2, open: true });
    const layout = createCollectionLayout(fakeStorage(saved));
    expect(visibleKeys(layout.prefs)).toEqual(["purchases", "profiles"]);
    expect(layout.prefs.open).toBe(true);
    expect(createCollectionLayout(fakeStorage("{nope")).prefs.split).toBe(4);
  });

  it("resets the order but keeps the open state", () => {
    const layout = createCollectionLayout(fakeStorage());
    layout.move(0, 6);
    layout.setOpen(true);
    layout.reset();
    expect(layout.prefs).toEqual({ order: [...COLLECTION_KEYS], split: 4, open: true });
  });
});
