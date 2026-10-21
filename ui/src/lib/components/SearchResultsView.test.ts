// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import type { Favorites } from "../favorites.svelte";
import type { Nav } from "../nav.svelte";
import type { PlaylistActions } from "../playlists.svelte";
import type { SearchResults, Track } from "../api";
import { createSearchPages } from "../searchPages.svelte";
import { fakePlayer } from "../test/fakes";
import type { Toasts } from "../toasts.svelte";
import SearchResultsView from "./SearchResultsView.svelte";

beforeAll(() => {
  const store = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
    removeItem: (k: string) => void store.delete(k),
  });
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
  globalThis.IntersectionObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof IntersectionObserver;
});

afterEach(cleanup);

const track = (id: number): Track => ({
  id,
  title: `Song ${id}`,
  duration: 200,
  artists: [{ id: 1, name: "AC/DC" }],
  trackNumber: 1,
  volumeNumber: 1,
  explicit: false,
});

function results(withTop: boolean): SearchResults {
  return {
    tracks: { items: Array.from({ length: 12 }, (_, i) => track(i + 1)), totalNumberOfItems: 300 },
    albums: { items: [{ id: 50, title: "Back In Black", artists: [{ id: 1, name: "AC/DC" }], numberOfTracks: 10, duration: 1 }] },
    artists: { items: [{ id: 1, name: "AC/DC" }] },
    playlists: { items: [] },
    topHit: withTop ? { kind: "album", item: { id: 50, title: "Back In Black", artists: [], numberOfTracks: 10, duration: 1 } } : null,
  };
}

function setup(r: SearchResults, query = "ac dc") {
  const pages = createSearchPages(async () => r);
  pages.seed(r, query);
  const onKind = vi.fn();
  render(SearchResultsView, {
    results: r,
    query,
    pages,
    kind: "all",
    onKind,
    nav: { goto: vi.fn(), route: { name: "explore" } } as unknown as Nav,
    player: fakePlayer(),
    favorites: { hasTrack: () => false, toggleTrack: vi.fn() } as unknown as Favorites,
    playlistActions: { openPicker: vi.fn() } as unknown as PlaylistActions,
    toasts: { show: vi.fn() } as unknown as Toasts,
  });
  return { onKind };
}

/// The carousels' titles in page order (they label their scroll regions).
const rowTitles = () =>
  screen
    .getAllByRole("region")
    .map((r) => r.getAttribute("aria-label"))
    .filter((label) => label !== null);

describe("SearchResultsView overview", () => {
  it("leads with the top hit, previews five tracks and puts its type's row first", () => {
    setup(results(true));
    expect(screen.getByText("Top result")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open Back In Black" })).toBeTruthy();
    expect(screen.getByText("Song 5")).toBeTruthy();
    expect(screen.queryByText("Song 6")).toBeNull();
    // The compact preview: no sort toolbar or column header.
    expect(screen.queryByRole("columnheader")).toBeNull();
    const rows = rowTitles();
    expect(rows).toEqual(["Albums", "Artists"]);
  });

  it("without a top hit, falls back to the artist named like the query", () => {
    setup({ ...results(false) });
    expect(screen.getByRole("button", { name: "Open AC/DC" })).toBeTruthy();
    expect(rowTitles()).toEqual(["Artists", "Albums"]);
  });

  it("shows more tracks without a top result, and See all switches to Tracks", async () => {
    const { onKind } = setup(results(false), "highway");
    expect(screen.queryByText("Top result")).toBeNull();
    expect(screen.getByText("Song 8")).toBeTruthy();
    expect(screen.queryByText("Song 9")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "See all tracks" }));
    expect(onKind).toHaveBeenCalledWith("tracks");
  });
});
