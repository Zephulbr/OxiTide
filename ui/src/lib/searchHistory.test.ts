import { describe, expect, it } from "vitest";
import { HISTORY_KEY, createSearchHistory } from "./searchHistory.svelte";

type HistoryStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

function fakeStorage(seed?: string): HistoryStorage {
  const map = new Map<string, string>();
  if (seed !== undefined) map.set(HISTORY_KEY, seed);
  return {
    getItem: (key) => map.get(key) ?? null,
    setItem: (key, value) => void map.set(key, value),
    removeItem: (key) => void map.delete(key),
  };
}

describe("search history store", () => {
  it("records queries most recent first", () => {
    const history = createSearchHistory(fakeStorage());
    history.record("blur");
    history.record("Stairway to");
    expect(history.entries).toEqual(["Stairway to", "blur"]);
  });

  it("trims entries and ignores blank queries", () => {
    const history = createSearchHistory(fakeStorage());
    history.record("  blur  ");
    history.record("   ");
    history.record("");
    expect(history.entries).toEqual(["blur"]);
  });

  it("moves a repeated query to the front without duplicating it", () => {
    const history = createSearchHistory(fakeStorage());
    history.record("blur");
    history.record("Stairway to");
    history.record("BLUR");
    expect(history.entries).toEqual(["BLUR", "Stairway to"]);
  });

  it("keeps at most eight entries, dropping the oldest", () => {
    const history = createSearchHistory(fakeStorage());
    for (let i = 1; i <= 10; i += 1) history.record(`query ${i}`);
    expect(history.entries).toHaveLength(8);
    expect(history.entries[0]).toBe("query 10");
    expect(history.entries[7]).toBe("query 3");
  });

  it("persists entries so a new store instance loads them", () => {
    const storage = fakeStorage();
    createSearchHistory(storage).record("blur");
    expect(createSearchHistory(storage).entries).toEqual(["blur"]);
  });

  it("starts empty when the stored value is corrupt", () => {
    const storage = fakeStorage("{not json");
    const history = createSearchHistory(storage);
    expect(history.entries).toEqual([]);
    history.record("blur");
    expect(history.entries).toEqual(["blur"]);
  });

  it("matches case-insensitive substrings, most recent first", () => {
    const history = createSearchHistory(fakeStorage());
    history.record("blur");
    history.record("Stairway to");
    history.record("blurred lines");
    expect(history.matches("BLU")).toEqual(["blurred lines", "blur"]);
    expect(history.matches("stair")).toEqual(["Stairway to"]);
    expect(history.matches("nomatch")).toEqual([]);
  });

  it("returns every entry for an empty match", () => {
    const history = createSearchHistory(fakeStorage());
    history.record("blur");
    history.record("Stairway to");
    expect(history.matches("")).toEqual(["Stairway to", "blur"]);
  });

  it("clears entries and the stored value", () => {
    const storage = fakeStorage();
    const history = createSearchHistory(storage);
    history.record("blur");
    history.clear();
    expect(history.entries).toEqual([]);
    expect(storage.getItem(HISTORY_KEY)).toBeNull();
  });
});
