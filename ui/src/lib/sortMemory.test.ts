import { describe, expect, it } from "vitest";
import { createSortMemory, MODE_KEY, SORTS_KEY } from "./sortMemory.svelte";

function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  return {
    data,
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => void data.set(key, value),
    removeItem: (key: string) => void data.delete(key),
  };
}

const ARTIST = [{ key: "artist" as const, dir: "asc" as const }];

describe("sortMemory", () => {
  it("remembers per page in memory by default, without touching storage", () => {
    const storage = fakeStorage();
    const memory = createSortMemory(storage);
    expect(memory.mode).toBe("session");
    memory.set("playlist:a", ARTIST);
    expect(memory.get("playlist:a")).toEqual(ARTIST);
    expect(memory.get("playlist:b")).toBeUndefined();
    expect(storage.data.has(SORTS_KEY)).toBe(false);
    memory.set("playlist:a", []);
    expect(memory.get("playlist:a")).toBeUndefined();
  });

  it("forgets everything in page mode", () => {
    const memory = createSortMemory(fakeStorage());
    memory.set("favorites", ARTIST);
    memory.setMode("page");
    expect(memory.get("favorites")).toBeUndefined();
    memory.set("favorites", ARTIST);
    expect(memory.get("favorites")).toBeUndefined();
  });

  it("persists in always mode and drops the saved sorts when leaving it", () => {
    const storage = fakeStorage();
    const memory = createSortMemory(storage);
    memory.set("favorites", ARTIST);
    memory.setMode("always");
    expect(storage.data.get(MODE_KEY)).toBe("always");
    expect(JSON.parse(storage.data.get(SORTS_KEY)!)).toEqual({ favorites: ARTIST });

    const reopened = createSortMemory(storage);
    expect(reopened.get("favorites")).toEqual(ARTIST);

    reopened.setMode("session");
    expect(storage.data.has(SORTS_KEY)).toBe(false);
  });

  it("ignores invalid stored data", () => {
    const storage = fakeStorage({
      [MODE_KEY]: "always",
      [SORTS_KEY]: JSON.stringify({
        good: ARTIST,
        badKey: [{ key: "nope", dir: "asc" }],
        dupes: [ARTIST[0], ARTIST[0]],
      }),
    });
    const memory = createSortMemory(storage);
    expect(memory.get("good")).toEqual(ARTIST);
    expect(memory.get("badKey")).toBeUndefined();
    expect(memory.get("dupes")).toBeUndefined();
    expect(createSortMemory(fakeStorage({ [MODE_KEY]: "bogus" })).mode).toBe("session");
  });
});
