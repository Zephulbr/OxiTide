import { describe, expect, it, vi } from "vitest";
import type { PlaylistViewPrefs } from "./playlistView.svelte";
import { playlistViewMenuItems } from "./playlistViewMenu";

function prefs(over: Partial<PlaylistViewPrefs> = {}): PlaylistViewPrefs {
  return { sort: "recent", dir: "desc", filter: "all", ...over };
}

function handlers() {
  return { setSort: vi.fn(), setFilter: vi.fn() };
}

describe("playlistViewMenuItems", () => {
  it("lists the sort rows under a Sort section in the image's order", () => {
    const items = playlistViewMenuItems(prefs(), handlers());
    const sortRows = items.filter((item) => item.section === "Sort");
    expect(sortRows.map((item) => item.label)).toEqual([
      "Recently played",
      "Created date",
      "Updated date",
      "Alphabetical",
    ]);
  });

  it("lists the filter rows under a Filter section", () => {
    const items = playlistViewMenuItems(prefs(), handlers());
    const filterRows = items.filter((item) => item.section === "Filter");
    expect(filterRows.map((item) => item.label)).toEqual([
      "All playlists",
      "Your playlists",
      "Followed playlists",
    ]);
  });

  it("marks only the active sort as selected", () => {
    const items = playlistViewMenuItems(prefs({ sort: "updated" }), handlers());
    const selected = items.filter((item) => item.section === "Sort" && item.selected);
    expect(selected.map((item) => item.label)).toEqual(["Updated date"]);
  });

  it("shows the direction arrow on the active sort row only", () => {
    const desc = playlistViewMenuItems(prefs({ sort: "updated", dir: "desc" }), handlers());
    expect(desc.find((item) => item.id === "sort-updated")?.trailing).toBe("arrow-down");
    expect(desc.find((item) => item.id === "sort-created")?.trailing).toBeUndefined();

    const asc = playlistViewMenuItems(prefs({ sort: "alpha", dir: "asc" }), handlers());
    expect(asc.find((item) => item.id === "sort-alpha")?.trailing).toBe("arrow-up");
  });

  it("marks the active filter as selected", () => {
    const items = playlistViewMenuItems(prefs({ filter: "followed" }), handlers());
    const selected = items.filter((item) => item.section === "Filter" && item.selected);
    expect(selected.map((item) => item.label)).toEqual(["Followed playlists"]);
  });

  it("runs the sort handler with the picked key", () => {
    const ctx = handlers();
    const items = playlistViewMenuItems(prefs(), ctx);
    items.find((item) => item.id === "sort-created")?.run();
    expect(ctx.setSort).toHaveBeenCalledWith("created");
  });

  it("runs the filter handler with the picked filter", () => {
    const ctx = handlers();
    const items = playlistViewMenuItems(prefs(), ctx);
    items.find((item) => item.id === "filter-owned")?.run();
    expect(ctx.setFilter).toHaveBeenCalledWith("owned");
  });
});
