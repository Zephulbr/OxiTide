// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";
import type { Favorites } from "../favorites.svelte";
import type { Toasts } from "../toasts.svelte";
import type { TrackView } from "../trackView.svelte";
import type { Track } from "../api";
import type { Nav } from "../nav.svelte";
import TrackTable from "./TrackTable.svelte";

vi.mock("../api", () => ({
  api: { playTracks: vi.fn(), queueAdd: vi.fn(async () => {}), queuePlayNext: vi.fn(async () => {}),getTrackFlags: vi.fn(async () => []) },
  trackToQueueItem: (t: Track) => t,
}));

// jsdom has no ResizeObserver; `bind:clientHeight` only needs it to exist.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;

afterEach(cleanup);

function track(id: number): Track {
  return {
    id,
    title: `Track ${id}`,
    duration: 200 + id,
    artists: [{ id: 10, name: "Artist" }],
    album: { id: 20, title: "Album", cover: null },
    trackNumber: id,
    volumeNumber: 1,
    explicit: false,
  } as Track;
}

function setup(overrides: Partial<TrackView> = {}) {
  const rows = [track(1), track(2), track(3)];
  const view = {
    filter: "",
    levels: [{ key: "title", dir: "asc" }],
    engaged: true,
    preparing: false,
    error: "",
    items: rows,
    setFilter: vi.fn(),
    toggleSort: vi.fn(),
    addSort: vi.fn(),
    flipSort: vi.fn(),
    removeSort: vi.fn(),
    moveSort: vi.fn(),
    clearSort: vi.fn(),
    retry: vi.fn(),
    sourceIndex: (index: number) => index,
    ...overrides,
  } as unknown as TrackView;
  const favorites = { hasTrack: () => false, toggleTrack: vi.fn() } as unknown as Favorites;
  const toasts = { show: vi.fn() } as unknown as Toasts;
  const onPlay = vi.fn();
  render(TrackTable, { view, favorites, toasts, onPlay, kind });
  return { rows, onPlay, view };
}

let kind: "favorites" | "playlist" | "album" | "artist" | "mix" | "search" = "search";

beforeEach(() => {
  kind = "search";
});

beforeEach(() => {
  vi.clearAllMocks();
});

describe("TrackTable semantics", () => {
  it("marks the sorted column with aria-sort and scopes the headers", () => {
    setup();
    const title = document.querySelector('th[aria-sort="ascending"]');
    expect(title?.textContent).toContain("Title");
    expect(document.querySelectorAll('thead th[scope="col"]').length).toBeGreaterThan(5);
  });

  it("splits a fill table's header out of the scroller, as one ARIA table", () => {
    const view = {
      filter: "", levels: [], engaged: false, preparing: false, error: "",
      items: [track(1), track(2)], setFilter: vi.fn(), toggleSort: vi.fn(), clearSort: vi.fn(),
      retry: vi.fn(), sourceIndex: (index: number) => index,
    } as unknown as TrackView;
    const favorites = { hasTrack: () => false, toggleTrack: vi.fn() } as unknown as Favorites;
    const toasts = { show: vi.fn() } as unknown as Toasts;
    render(TrackTable, { view, favorites, toasts, fill: true, onAddToPlaylist: vi.fn() });

    const frame = document.querySelector<HTMLElement>('.table-frame[role="table"]');
    expect(frame?.getAttribute("aria-label")).toBe("Tracks");
    expect(frame?.style.getPropertyValue("--actions")).toBe("3");
    // The header is not inside the masked scroller.
    expect(document.querySelector(".table-scroll thead")).toBeNull();
    const heads = frame!.querySelectorAll('.table-head th[role="columnheader"]');
    expect(heads.length).toBeGreaterThan(5);
    // Both tables size from the same columns, one per header cell.
    for (const colgroup of frame!.querySelectorAll("colgroup")) {
      expect(colgroup.children).toHaveLength(heads.length);
    }
    const rows = frame!.querySelectorAll('.table-scroll tr.row[role="row"]');
    expect(rows).toHaveLength(2);
    expect(rows[0].querySelectorAll('td[role="cell"]')).toHaveLength(heads.length);
    for (const table of frame!.querySelectorAll("table")) {
      expect(table.getAttribute("role")).toBe("presentation");
    }
  });

  it("keeps a plain table with its own header when not filling", () => {
    setup();
    expect(document.querySelector(".table-frame")).toBeNull();
    expect(document.querySelector('.table-scroll table[aria-label="Tracks"] thead')).not.toBeNull();
    expect(document.querySelector("tr[role]")).toBeNull();
  });

  it("swaps the playing row's number for the now-playing mark", () => {
    const view = {
      filter: "", levels: [{ key: "title", dir: "asc" }], engaged: true, preparing: false, error: "",
      items: [track(1), track(2)], setFilter: vi.fn(), toggleSort: vi.fn(), clearSort: vi.fn(),
      retry: vi.fn(), sourceIndex: (index: number) => index,
    } as unknown as TrackView;
    const favorites = { hasTrack: () => false, toggleTrack: vi.fn() } as unknown as Favorites;
    const toasts = { show: vi.fn() } as unknown as Toasts;
    render(TrackTable, { view, favorites, toasts, onPlay: vi.fn(), playingTrackId: 2, playbackRunning: true });

    const cells = document.querySelectorAll("tbody tr.row td.num-cell");
    expect(cells[0].textContent?.trim()).toBe("1");
    expect(cells[1].querySelector('[role="img"]')?.getAttribute("aria-label")).toBe("Now playing");
  });

  it("offers 'Now playing' only when the playing track is listed and scrolls to it", async () => {
    const view = {
      filter: "", levels: [], engaged: false, preparing: false, error: "",
      items: [track(1), track(2), track(3)], setFilter: vi.fn(), toggleSort: vi.fn(), clearSort: vi.fn(),
      retry: vi.fn(), sourceIndex: (index: number) => index,
    } as unknown as TrackView;
    const favorites = { hasTrack: () => false, toggleTrack: vi.fn() } as unknown as Favorites;
    const toasts = { show: vi.fn() } as unknown as Toasts;
    const scrollTo = vi.fn();
    const original = HTMLElement.prototype.scrollTo;
    HTMLElement.prototype.scrollTo = scrollTo;
    try {
      const { rerender } = render(TrackTable, { view, favorites, toasts, playingTrackId: 9 });
      expect(document.querySelector("button.locate")).toBeNull();

      await rerender({ view, favorites, toasts, playingTrackId: 3 });
      await fireEvent.click(document.querySelector<HTMLButtonElement>("button.locate")!);
      expect(scrollTo).toHaveBeenCalledTimes(1);
      expect(scrollTo.mock.calls[0][0].top).toBeGreaterThan(0);
    } finally {
      HTMLElement.prototype.scrollTo = original;
    }
  });

  it("labels virtualized rows with their index in the full list", () => {
    setup();
    const rows = document.querySelectorAll<HTMLTableRowElement>("tbody tr.row");
    expect(rows).toHaveLength(3);
    expect(rows[0].getAttribute("aria-rowindex")).toBe("2");
    expect(rows[2].getAttribute("aria-rowindex")).toBe("4");
    expect(document.querySelector("table")?.getAttribute("aria-rowcount")).toBe("4");
  });

  it("keeps exactly one tab stop and moves it with arrows, Home, and End", async () => {
    setup();
    const rows = document.querySelectorAll<HTMLTableRowElement>("tbody tr.row");
    expect([...rows].map((row) => row.tabIndex)).toEqual([0, -1, -1]);

    rows[0].focus();
    await fireEvent.keyDown(rows[0], { key: "ArrowDown" });
    await waitFor(() => expect(document.activeElement).toBe(rows[1]));
    expect([...rows].map((row) => row.tabIndex)).toEqual([-1, 0, -1]);

    await fireEvent.keyDown(rows[1], { key: "End" });
    await waitFor(() => expect(document.activeElement).toBe(rows[2]));

    await fireEvent.keyDown(rows[2], { key: "Home" });
    await waitFor(() => expect(document.activeElement).toBe(rows[0]));
  });

  it("plays the focused row on Enter but leaves keys on row buttons alone", async () => {
    const { onPlay } = setup();
    const rows = document.querySelectorAll<HTMLTableRowElement>("tbody tr.row");

    rows[1].focus();
    await fireEvent.keyDown(rows[1], { key: "Enter" });
    expect(onPlay).toHaveBeenCalledWith(1);

    const heart = rows[2].querySelector<HTMLButtonElement>('[aria-label="Favorite"]')!;
    await fireEvent.keyDown(heart, { key: "Enter" });
    expect(onPlay).toHaveBeenCalledTimes(1);
  });
});

describe("TrackTable name links", () => {
  function renderWithNav(route: Nav["route"]) {
    const view = {
      filter: "", levels: [], engaged: false, preparing: false, error: "",
      items: [track(1)], setFilter: vi.fn(), toggleSort: vi.fn(), clearSort: vi.fn(),
      retry: vi.fn(), sourceIndex: (index: number) => index,
    } as unknown as TrackView;
    const favorites = { hasTrack: () => false, toggleTrack: vi.fn() } as unknown as Favorites;
    const toasts = { show: vi.fn() } as unknown as Toasts;
    const nav = { route, goto: vi.fn() } as unknown as Nav;
    const onPlay = vi.fn();
    render(TrackTable, { view, favorites, toasts, onPlay, nav });
    const links = [...document.querySelectorAll<HTMLElement>('tbody [role="link"]')];
    return { nav, onPlay, links };
  }

  it("links title and album to the album and the artist to the artist", async () => {
    const { nav, links } = renderWithNav({ name: "playlist", id: "p" });
    expect(links.map((l) => l.textContent)).toEqual(["Track 1", "Artist", "Album"]);
    expect(links.every((l) => l.tabIndex === -1)).toBe(true);

    await fireEvent.click(links[1]);
    expect(nav.goto).toHaveBeenCalledWith({ name: "artist", id: 10 });
    await fireEvent.click(links[0]);
    expect(nav.goto).toHaveBeenLastCalledWith({ name: "album", id: 20 });
  });

  it("does not link the page's own album or artist", () => {
    expect(renderWithNav({ name: "album", id: 20 }).links.map((l) => l.textContent)).toEqual(["Artist"]);
    cleanup();
    expect(renderWithNav({ name: "artist", id: 10 }).links.map((l) => l.textContent)).toEqual(["Track 1", "Album"]);
  });

  it("does not play the row when a name is double-clicked", async () => {
    const { onPlay, links } = renderWithNav({ name: "playlist", id: "p" });
    await fireEvent.dblClick(links[1]);
    expect(onPlay).not.toHaveBeenCalled();
  });

  it("renders plain text without nav", () => {
    setup();
    expect(document.querySelectorAll('tbody [role="link"]')).toHaveLength(0);
  });
});

describe("TrackTable multi-level sorting", () => {
  const header = (label: string) =>
    [...document.querySelectorAll<HTMLElement>("thead th")].find((th) =>
      th.querySelector("button.sort")?.textContent?.includes(label),
    )!;

  it("offers 'then sort by' on other headers and adds the level", async () => {
    const { view } = setup();
    const artist = header("Artist");
    const plus = artist.querySelector<HTMLButtonElement>('button[aria-label="Then sort by Artist"]')!;
    await fireEvent.click(plus);
    expect(view.addSort).toHaveBeenCalledWith("artist");
    expect(header("Title").querySelector('[aria-label="Then sort by Title"]')).toBeNull();
  });

  it("adds a level on Shift+click and replaces on a plain click", async () => {
    const { view } = setup();
    const albumButton = header("Album").querySelector<HTMLButtonElement>("button.sort")!;
    await fireEvent.click(albumButton, { shiftKey: true });
    expect(view.addSort).toHaveBeenCalledWith("album");
    await fireEvent.click(albumButton);
    expect(view.toggleSort).toHaveBeenCalledWith("album");
  });

  it("shows rank badges and chips for every level", async () => {
    const { view } = setup({
      levels: [
        { key: "artist", dir: "asc" },
        { key: "title", dir: "desc" },
      ],
    });
    expect(header("Artist").querySelector(".rank")?.textContent).toBe("1");
    expect(header("Title").querySelector(".rank")?.textContent).toBe("2");
    expect(header("Artist").getAttribute("aria-sort")).toBe("ascending");
    expect(header("Title").getAttribute("aria-sort")).toBeNull();

    const chips = document.querySelectorAll<HTMLButtonElement>(".sort-bar .chip-main");
    expect([...chips].map((chip) => chip.textContent?.replace(/\s+/g, " ").trim())).toEqual([
      "1 Artist",
      "2 Title",
    ]);
    await fireEvent.click(chips[1]);
    expect(view.flipSort).toHaveBeenCalledWith(1);
    await fireEvent.keyDown(chips[1], { key: "ArrowLeft", altKey: true });
    expect(view.moveSort).toHaveBeenCalledWith(1, 0);
    await fireEvent.click(document.querySelector<HTMLButtonElement>('[aria-label="Remove Artist sort"]')!);
    expect(view.removeSort).toHaveBeenCalledWith(0);
  });

  it("shows date added in favorites, and a sorted optional column", () => {
    kind = "favorites";
    const rows = [
      { ...track(1), dateAdded: "2025-08-06T17:50:35.372+0000", popularity: 71 },
    ] as Track[];
    setup({ items: rows, levels: [{ key: "popularity", dir: "desc" }] });
    const heads = [...document.querySelectorAll("thead th")].map((th) => th.textContent?.trim());
    expect(heads).toContain("Date added");
    expect(heads.some((text) => text?.startsWith("Popularity"))).toBe(true);
    expect(heads).not.toContain("Plays");
    const cells = [...document.querySelectorAll("tbody td.opt-col")].map((td) => td.textContent);
    expect(cells).toEqual(["Aug 6, 2025", "71"]);
  });

  it("does not offer date added outside favorites and playlists", () => {
    setup();
    expect(document.querySelector('[aria-label="Then sort by Date added"]')).toBeNull();
    expect([...document.querySelectorAll("thead th")].map((th) => th.textContent?.trim())).not.toContain(
      "Date added",
    );
  });
});
