import { describe, expect, it } from "vitest";
import { createHomeSearch, HOME_SEARCH_KEY } from "./homeSearch.svelte";

function memory(initial: Record<string, string> = {}) {
  const store = new Map(Object.entries(initial));
  return {
    store,
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
  };
}

describe("homeSearch", () => {
  it("is shown by default", () => {
    expect(createHomeSearch(memory()).shown).toBe(true);
    expect(createHomeSearch(null).shown).toBe(true);
  });

  it("reads a stored choice", () => {
    expect(createHomeSearch(memory({ [HOME_SEARCH_KEY]: "off" })).shown).toBe(false);
    expect(createHomeSearch(memory({ [HOME_SEARCH_KEY]: "on" })).shown).toBe(true);
    expect(createHomeSearch(memory({ [HOME_SEARCH_KEY]: "junk" })).shown).toBe(true);
  });

  it("persists a change", () => {
    const storage = memory();
    const flag = createHomeSearch(storage);
    flag.setShown(false);
    expect(flag.shown).toBe(false);
    expect(storage.store.get(HOME_SEARCH_KEY)).toBe("off");
    expect(createHomeSearch(storage).shown).toBe(false);
  });

  it("survives a storage that throws", () => {
    const broken = {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    };
    const flag = createHomeSearch(broken);
    expect(flag.shown).toBe(true);
    flag.setShown(false);
    expect(flag.shown).toBe(false);
  });
});
