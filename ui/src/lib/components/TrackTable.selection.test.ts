// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/svelte";
import type { Favorites } from "../favorites.svelte";
import type { Toasts } from "../toasts.svelte";
import type { TrackView } from "../trackView.svelte";
import type { Track } from "../api";
import { appearance } from "../appearance.svelte";
import { dragDrop } from "../dragDrop.svelte";
import { menu } from "../menu.svelte";
import { rowHeightFor } from "../rowHeight";
import TrackTable from "./TrackTable.svelte";

vi.mock("../api", () => ({
  api: {
    playTracks: vi.fn(),
    queueAdd: vi.fn(async () => {}),
    queuePlayNext: vi.fn(async () => {}),
    getTrackFlags: vi.fn(async () => []),
  },
  trackToQueueItem: (t: Track) => t,
}));

globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;

function track(id: number): Track {
  return {
    id,
    title: `Track ${id}`,
    duration: 200,
    artists: [{ id: 10, name: "Artist" }],
    album: { id: 20, title: "Album", cover: null },
    trackNumber: id,
    volumeNumber: 1,
    explicit: false,
  } as Track;
}

function makeView(items: Track[], overrides: Partial<TrackView> = {}): TrackView {
  return {
    filter: "",
    levels: [],
    engaged: false,
    preparing: false,
    error: "",
    items,
    setFilter: vi.fn(),
    toggleSort: vi.fn(),
    addSort: vi.fn(),
    clearSort: vi.fn(),
    retry: vi.fn(),
    sourceIndex: (index: number) => index,
    ...overrides,
  } as unknown as TrackView;
}

const favorites = { hasTrack: () => false, toggleTrack: vi.fn() } as unknown as Favorites;
const toasts = { show: vi.fn() } as unknown as Toasts;

function setup(props: Record<string, unknown> = {}, ids = [1, 2, 3, 4]) {
  const rows = ids.map(track);
  const view = makeView(rows);
  const result = render(TrackTable, { view, favorites, toasts, kind: "playlist", ...props });
  const rowEl = (i: number) => document.querySelector<HTMLElement>(`tr.row[data-row-index="${i}"]`)!;
  const selected = () =>
    [...document.querySelectorAll<HTMLElement>("tr.row.selected")].map((el) => Number(el.dataset.rowIndex));
  return { rows, view, rowEl, selected, ...result };
}

function pointer(type: string, target: EventTarget, x: number, y: number) {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0 });
  Object.defineProperty(event, "pointerType", { value: "mouse" });
  target.dispatchEvent(event);
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "requestAnimationFrame", "cancelAnimationFrame"] });
});

afterEach(() => {
  dragDrop.cancel();
  // A cancelled drag waits for the button's release before it stops
  // swallowing the click; release it so the next test starts clean.
  pointer("pointerup", window, 0, 0);
  menu.close(false);
  vi.runOnlyPendingTimers();
  vi.useRealTimers();
  cleanup();
});

describe("TrackTable selection", () => {
  it("a press outside the rows clears it; rows, menus and dialogs keep it", async () => {
    const { rowEl, selected } = setup();
    await fireEvent.click(rowEl(1));
    await fireEvent.click(rowEl(2), { ctrlKey: true });

    pointer("pointerdown", rowEl(3), 0, 0);
    expect(selected()).toEqual([1, 2]);
    for (const role of ["menu", "dialog"]) {
      const layer = document.createElement("div");
      layer.setAttribute("role", role);
      layer.append(document.createElement("button"));
      document.body.append(layer);
      pointer("pointerdown", layer.firstElementChild!, 0, 0);
      layer.remove();
    }
    await Promise.resolve();
    expect(selected()).toEqual([1, 2]);

    pointer("pointerdown", document.body, 0, 0);
    await Promise.resolve();
    expect(selected()).toEqual([]);
  });

  it("click selects, Ctrl toggles, Shift ranges", async () => {
    const { rowEl, selected } = setup();
    await fireEvent.click(rowEl(1));
    expect(selected()).toEqual([1]);
    await fireEvent.click(rowEl(3), { ctrlKey: true });
    expect(selected()).toEqual([1, 3]);
    await fireEvent.click(rowEl(1), { ctrlKey: true });
    expect(selected()).toEqual([3]);
    // The last Ctrl-click (row 1) is the anchor the range starts from.
    await fireEvent.click(rowEl(0), { shiftKey: true });
    expect(selected()).toEqual([0, 1]);
    await fireEvent.click(rowEl(3), { shiftKey: true });
    expect(selected()).toEqual([1, 2, 3]);
  });

  it("clicks on the row's buttons do not select", async () => {
    const { rowEl, selected } = setup();
    await fireEvent.click(rowEl(2).querySelector("button.thumb")!);
    expect(selected()).toEqual([]);
  });

  it("Ctrl+A selects all, Escape clears, Shift+Down extends", async () => {
    const { rowEl, selected } = setup();
    await fireEvent.keyDown(rowEl(0), { key: "a", ctrlKey: true });
    expect(selected()).toEqual([0, 1, 2, 3]);
    await fireEvent.keyDown(rowEl(0), { key: "Escape" });
    expect(selected()).toEqual([]);
    await fireEvent.keyDown(rowEl(1), { key: "ArrowDown", shiftKey: true });
    expect(selected()).toEqual([1, 2]);
  });

  it("starts over when the order changes, but not for the same view", async () => {
    const { rowEl, selected, rows, rerender } = setup();
    await fireEvent.click(rowEl(1));
    expect(selected()).toEqual([1]);
    await rerender({ view: makeView(rows, { levels: [{ key: "title", dir: "asc" }] } as Partial<TrackView>) });
    expect(selected()).toEqual([]);
  });

  it("right-click in a selection opens the selection menu", async () => {
    const onAddManyToPlaylist = vi.fn();
    const { rowEl, rows } = setup({ onAddManyToPlaylist });
    await fireEvent.click(rowEl(0));
    await fireEvent.click(rowEl(2), { ctrlKey: true });
    await fireEvent.contextMenu(rowEl(2));
    const items = menu.open?.items ?? [];
    expect(items.map((i) => i.label)).toContain("Add 2 to playlist");
    items.find((i) => i.id === "add-playlist")!.run();
    expect(onAddManyToPlaylist).toHaveBeenCalledWith([rows[0], rows[2]]);
  });

  it("right-click outside the selection selects that row and shows its own menu", async () => {
    const { rowEl, selected } = setup();
    await fireEvent.click(rowEl(0));
    await fireEvent.click(rowEl(1), { ctrlKey: true });
    await fireEvent.contextMenu(rowEl(3));
    expect(selected()).toEqual([3]);
    expect(menu.open?.items.map((i) => i.id)).toContain("track-info");
  });

  it("offers Remove N for an editable playlist and passes displayed indices", async () => {
    const onRemoveMany = vi.fn();
    const { rowEl } = setup({
      playlistActions: { onRemove: vi.fn(), onRemoveMany, busy: false, disabled: false },
    });
    await fireEvent.click(rowEl(1));
    await fireEvent.click(rowEl(3), { ctrlKey: true });
    await fireEvent.contextMenu(rowEl(1));
    menu.open!.items.find((i) => i.id === "remove")!.run();
    expect(onRemoveMany).toHaveBeenCalledWith([1, 3]);
  });
});

describe("TrackTable drag", () => {
  it("a drag from a selected row carries the selection in display order", async () => {
    const { rowEl, rows } = setup({ sourcePlaylist: "p1" });
    await fireEvent.click(rowEl(3));
    await fireEvent.click(rowEl(1), { ctrlKey: true });
    pointer("pointerdown", rowEl(3).querySelector("td")!, 0, 0);
    pointer("pointermove", window, 30, 30);
    const payload = dragDrop.active;
    expect(payload?.kind).toBe("tracks");
    if (payload?.kind !== "tracks") return;
    expect(payload.tracks).toEqual([rows[1], rows[3]]);
    expect(payload.source?.playlistId).toBe("p1");
  });

  it("a drag from an unselected row carries that row alone", async () => {
    const { rowEl, rows, selected } = setup();
    await fireEvent.click(rowEl(0));
    pointer("pointerdown", rowEl(2).querySelector("td")!, 0, 0);
    pointer("pointermove", window, 30, 30);
    const payload = dragDrop.active;
    expect(payload?.kind === "tracks" && payload.tracks).toEqual([rows[2]]);
    expect(selected()).toEqual([0]);
  });

  it("reorders one row of an editable playlist in place", async () => {
    const onDrop = vi.fn();
    const { rowEl } = setup({
      playlistActions: { onRemove: vi.fn(), busy: false, disabled: false, onDrop },
    });
    const scroller = document.querySelector<HTMLElement>(".table-scroll")!;
    document.elementFromPoint = vi.fn(() => scroller);
    const rowHeight = rowHeightFor(appearance.density);
    pointer("pointerdown", rowEl(0).querySelector("td")!, 0, 0);
    pointer("pointermove", window, 0, rowHeight * 3);
    expect(dragDrop.verdict).toEqual({ ok: true, hint: "Move" });
    pointer("pointerup", window, 0, rowHeight * 3);
    expect(onDrop).toHaveBeenCalledWith(0, 2);
  });

  it("refuses to move several rows yet", async () => {
    const onDrop = vi.fn();
    const { rowEl } = setup({
      playlistActions: { onRemove: vi.fn(), busy: false, disabled: false, onDrop },
    });
    document.elementFromPoint = vi.fn(() => document.querySelector(".table-scroll"));
    await fireEvent.click(rowEl(0));
    await fireEvent.click(rowEl(1), { ctrlKey: true });
    pointer("pointerdown", rowEl(0).querySelector("td")!, 0, 0);
    pointer("pointermove", window, 0, 200);
    expect(dragDrop.verdict?.ok).toBe(false);
    pointer("pointerup", window, 0, 200);
    expect(onDrop).not.toHaveBeenCalled();
  });

  it("a table that cannot reorder is no target for its own rows", async () => {
    const { rowEl } = setup();
    document.elementFromPoint = vi.fn(() => document.querySelector(".table-scroll"));
    pointer("pointerdown", rowEl(0).querySelector("td")!, 0, 0);
    pointer("pointermove", window, 0, 200);
    expect(dragDrop.active).not.toBeNull();
    expect(dragDrop.targetKey).toBeNull();
  });
});
