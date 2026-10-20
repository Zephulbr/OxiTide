import { describe, expect, it, vi } from "vitest";
import type { Album, SearchResults } from "./api";
import { createSearchPages } from "./searchPages.svelte";

const album = (id: number): Album => ({ id, title: `A${id}`, artists: [], numberOfTracks: 1, duration: 1 });

function results(albums: Album[], total: number): SearchResults {
  return {
    tracks: { items: [] },
    albums: { items: albums, totalNumberOfItems: total },
    artists: { items: [] },
    playlists: { items: [] },
  };
}

describe("createSearchPages", () => {
  it("seeds from the overview and appends the next page at its offset", async () => {
    const fetchPage = vi.fn(async () => results([album(2), album(3)], 3));
    const pages = createSearchPages(fetchPage);
    pages.seed(results([album(1), album(2)], 3), "beck");
    expect(pages.items("albums").map((a) => a.id)).toEqual([1, 2]);
    expect(pages.hasMore("albums")).toBe(true);
    await pages.more("albums");
    expect(fetchPage).toHaveBeenCalledWith("beck", "albums", 2);
    expect(pages.items("albums").map((a) => a.id)).toEqual([1, 2, 3]);
    expect(pages.hasMore("albums")).toBe(false);
  });

  it("ends the list on a page with nothing new", async () => {
    const pages = createSearchPages(async () => results([album(1)], 99));
    pages.seed(results([album(1)], 99), "x");
    await pages.more("albums");
    expect(pages.hasMore("albums")).toBe(false);
  });

  it("drops a page that lands after a new search", async () => {
    let release: (r: SearchResults) => void = () => {};
    const pages = createSearchPages(() => new Promise((resolve) => (release = resolve)));
    pages.seed(results([album(1)], 5), "old");
    const pending = pages.more("albums");
    pages.seed(results([album(7)], 1), "new");
    release(results([album(2)], 5));
    await pending;
    expect(pages.items("albums").map((a) => a.id)).toEqual([7]);
    expect(pages.loading("albums")).toBe(false);
  });

  it("keeps an error for retry", async () => {
    const pages = createSearchPages(async () => {
      throw new Error("offline");
    });
    pages.seed(results([album(1)], 5), "x");
    await pages.more("albums");
    expect(pages.error("albums")).toContain("offline");
    expect(pages.hasMore("albums")).toBe(true);
  });
});
