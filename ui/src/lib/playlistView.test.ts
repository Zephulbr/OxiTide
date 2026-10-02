import { describe, expect, it } from "vitest";
import { VIEW_KEY, createPlaylistView } from "./playlistView.svelte";

type ViewStorage = Pick<Storage, "getItem" | "setItem">;

function fakeStorage(seed?: string): ViewStorage {
  const map = new Map<string, string>();
  if (seed !== undefined) map.set(VIEW_KEY, seed);
  return {
    getItem: (key) => map.get(key) ?? null,
    setItem: (key, value) => void map.set(key, value),
  };
}

describe("playlist view store", () => {
  it("starts with recently played first and no filter", () => {
    const view = createPlaylistView(fakeStorage());
    expect(view.prefs).toEqual({ sort: "recent", dir: "desc", filter: "all" });
  });

  it("uses the key's default direction when switching sort", () => {
    const view = createPlaylistView(fakeStorage());
    view.setSort("created");
    expect(view.prefs).toEqual({ sort: "created", dir: "desc", filter: "all" });
    view.setSort("updated");
    expect(view.prefs).toEqual({ sort: "updated", dir: "desc", filter: "all" });
    view.setSort("alpha");
    expect(view.prefs).toEqual({ sort: "alpha", dir: "asc", filter: "all" });
    view.setSort("recent");
    expect(view.prefs).toEqual({ sort: "recent", dir: "desc", filter: "all" });
  });

  it("toggles direction when the active sort is picked again", () => {
    const view = createPlaylistView(fakeStorage());
    view.setSort("alpha");
    view.setSort("alpha");
    expect(view.prefs.dir).toBe("desc");
    view.setSort("alpha");
    expect(view.prefs.dir).toBe("asc");
  });

  it("sets the filter", () => {
    const view = createPlaylistView(fakeStorage());
    view.setFilter("followed");
    expect(view.prefs.filter).toBe("followed");
  });

  it("persists prefs so a new store instance loads them", () => {
    const storage = fakeStorage();
    const view = createPlaylistView(storage);
    view.setSort("updated");
    view.setFilter("owned");
    expect(createPlaylistView(storage).prefs).toEqual({
      sort: "updated",
      dir: "desc",
      filter: "owned",
    });
  });

  it("starts with defaults when the stored value is corrupt", () => {
    const view = createPlaylistView(fakeStorage("{not json"));
    expect(view.prefs).toEqual({ sort: "recent", dir: "desc", filter: "all" });
  });

  it("falls back per field for unknown stored values", () => {
    const view = createPlaylistView(
      fakeStorage(JSON.stringify({ sort: "bogus", dir: "sideways", filter: "owned" })),
    );
    expect(view.prefs).toEqual({ sort: "recent", dir: "desc", filter: "owned" });
  });
});
