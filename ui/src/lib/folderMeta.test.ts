import { describe, expect, it } from "vitest";
import { createFolderMeta, DESCRIPTION_MAX, FOLDER_META_KEY } from "./folderMeta.svelte";

function fakeStorage(initial: Record<string, string> = {}) {
  const items = new Map(Object.entries(initial));
  return {
    items,
    getItem: (key: string) => items.get(key) ?? null,
    setItem: (key: string, value: string) => void items.set(key, value),
  };
}

const stored = (storage: ReturnType<typeof fakeStorage>) => JSON.parse(storage.items.get(FOLDER_META_KEY) ?? "{}");

describe("folderMeta", () => {
  it("sets, persists and clears a folder colour", () => {
    const storage = fakeStorage();
    const meta = createFolderMeta(storage);
    expect(meta.hue("f1")).toBe(null);
    meta.setColor("f1", "blue");
    expect(meta.color("f1")).toBe("blue");
    expect(meta.hue("f1")).toBe(250);
    expect(stored(storage)).toEqual({ f1: { color: "blue" } });
    meta.setColor("f1", null);
    expect(meta.color("f1")).toBe(null);
    expect(stored(storage)).toEqual({});
  });

  it("keeps colour and description side by side; a blank description clears", () => {
    const storage = fakeStorage();
    const meta = createFolderMeta(storage);
    meta.setColor("f", "green");
    meta.setDescription("f", "  Late-night picks  ");
    expect(meta.description("f")).toBe("Late-night picks");
    expect(stored(storage)).toEqual({ f: { color: "green", description: "Late-night picks" } });
    meta.setDescription("f", "   ");
    expect(meta.description("f")).toBe("");
    expect(meta.color("f")).toBe("green");
    meta.setDescription("g", "x".repeat(DESCRIPTION_MAX + 50));
    expect(meta.description("g")).toHaveLength(DESCRIPTION_MAX);
  });

  it("ignores malformed values and reads the old colours-only key", () => {
    const storage = fakeStorage({
      [FOLDER_META_KEY]: JSON.stringify({ a: { color: "neon", description: 4 }, b: "teal", c: { description: "Hi" } }),
      "oxitide.folder-colors": JSON.stringify({ d: "pink", e: "nope" }),
    });
    const meta = createFolderMeta(storage);
    expect(meta.color("a")).toBe(null);
    expect(meta.color("b")).toBe(null);
    expect(meta.description("c")).toBe("Hi");
    expect(meta.color("d")).toBe("pink");
    expect(meta.color("e")).toBe(null);
    expect(createFolderMeta(fakeStorage({ [FOLDER_META_KEY]: "{oops" })).color("a")).toBe(null);
  });
});
