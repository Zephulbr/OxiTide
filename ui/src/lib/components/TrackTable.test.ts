// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";
import type { Favorites } from "../favorites.svelte";
import type { Toasts } from "../toasts.svelte";
import type { TrackView } from "../trackView.svelte";
import type { Track } from "../api";
import TrackTable from "./TrackTable.svelte";

vi.mock("../api", () => ({
  api: { playTracks: vi.fn(), queueAdd: vi.fn(), getTrackFlags: vi.fn(async () => []) },
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
    sort: "title",
    direction: "asc",
    engaged: true,
    preparing: false,
    error: "",
    items: rows,
    setFilter: vi.fn(),
    toggleSort: vi.fn(),
    clearSort: vi.fn(),
    retry: vi.fn(),
    sourceIndex: (index: number) => index,
    ...overrides,
  } as unknown as TrackView;
  const favorites = { hasTrack: () => false, toggleTrack: vi.fn() } as unknown as Favorites;
  const toasts = { show: vi.fn() } as unknown as Toasts;
  const onPlay = vi.fn();
  render(TrackTable, { view, favorites, toasts, onPlay });
  return { rows, onPlay };
}

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
