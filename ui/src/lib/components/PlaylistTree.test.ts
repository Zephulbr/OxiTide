// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import { tick } from "svelte";
import type { CollectionEntry, Track } from "../api";
import type { CollectionActions } from "../collectionActions.svelte";
import type { CollectionTree } from "../collection.svelte";
import { dragDrop, dragSource } from "../dragDrop.svelte";
import type { DragPayload } from "../dragPayload";
import type { Nav } from "../nav.svelte";
import type { PlaylistActions } from "../playlists.svelte";
import PlaylistTree from "./PlaylistTree.svelte";

vi.mock("../api", () => ({
  api: {
    setPlaySource: vi.fn(async () => {}),
    // Owned playlists draw a collage from their first tracks.
    getPlaylistTracksPage: vi.fn(async () => ({ items: [], total: 0 })),
  },
}));
vi.mock("../playlistView.svelte", () => ({
  playlistView: () => ({ prefs: { sort: "alpha", dir: "asc", filter: "all" } }),
}));

const tracks = [{ id: 1, title: "T1" }] as Track[];

function entries(): CollectionEntry[] {
  return [
    { kind: "folder", id: "f1", name: "Folder", totalNumberOfItems: 1 },
    { kind: "playlist", uuid: "mine", title: "Mine", owned: true, numberOfTracks: 2 },
    { kind: "playlist", uuid: "theirs", title: "Theirs", owned: false, numberOfTracks: 5 },
  ] as CollectionEntry[];
}

function setup() {
  const collection = {
    children: () => null,
    isExpanded: vi.fn(() => false),
    isLoading: () => false,
    toggleFolder: vi.fn(async () => {}),
  } as unknown as CollectionTree;
  const nav = { route: { name: "home" }, goto: vi.fn() } as unknown as Nav;
  const actions = {} as CollectionActions;
  const playlistActions = { addTracks: vi.fn(async () => {}) } as unknown as PlaylistActions;
  render(PlaylistTree, { entries: entries(), collection, nav, actions, playlistActions });
  const source = document.createElement("div");
  document.body.append(source);
  const handle = dragSource(source, () => payload);
  cleanups.push(() => handle.destroy());
  return { collection, nav, playlistActions, source };
}

let payload: DragPayload = { kind: "tracks", tracks };
let cleanups: (() => void)[] = [];

function pointer(type: string, target: EventTarget, x = 0, y = 0) {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0 });
  Object.defineProperty(event, "pointerType", { value: "mouse" });
  target.dispatchEvent(event);
}

function row(title: string): HTMLElement {
  return document.querySelector<HTMLElement>(`button.row[title="${title}"]`)!;
}

/// Drag from the stand-in source onto `target`.
function dragOnto(source: HTMLElement, target: Element) {
  document.elementFromPoint = vi.fn(() => target);
  pointer("pointerdown", source);
  pointer("pointermove", window, 30, 30);
}

beforeEach(() => {
  payload = { kind: "tracks", tracks };
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "requestAnimationFrame", "cancelAnimationFrame"] });
});

afterEach(() => {
  dragDrop.cancel();
  pointer("pointerup", window);
  vi.runOnlyPendingTimers();
  vi.useRealTimers();
  for (const done of cleanups) done();
  cleanups = [];
  cleanup();
});

describe("PlaylistTree drops", () => {
  it("adds to an owned playlist and marks it as the target", async () => {
    const { source, playlistActions } = setup();
    dragOnto(source, row("Mine"));
    await tick();
    expect(row("Mine").classList.contains("drop-target")).toBe(true);
    expect(dragDrop.verdict).toEqual({ ok: true, hint: "Add to Mine" });
    pointer("pointerup", window, 30, 30);
    expect(playlistActions.addTracks).toHaveBeenCalledTimes(1);
    const [target] = vi.mocked(playlistActions.addTracks).mock.calls[0];
    expect(target).toMatchObject({ uuid: "mine" });
  });

  it("refuses someone else's playlist without calling the API", async () => {
    const { source, playlistActions } = setup();
    dragOnto(source, row("Theirs"));
    await tick();
    expect(row("Theirs").classList.contains("drop-refused")).toBe(true);
    expect(dragDrop.verdict).toEqual({ ok: false, reason: "You can't edit this playlist" });
    pointer("pointerup", window, 30, 30);
    expect(playlistActions.addTracks).not.toHaveBeenCalled();
  });

  it("refuses the playlist the rows came from", () => {
    payload = { kind: "tracks", tracks, source: { playlistId: "mine" } };
    const { source } = setup();
    dragOnto(source, row("Mine"));
    expect(dragDrop.verdict?.ok).toBe(false);
  });

  it("springs a collapsed folder open after a rest, not on a pass", () => {
    const { source, collection } = setup();
    dragOnto(source, row("Folder"));
    vi.advanceTimersByTime(300);
    expect(collection.toggleFolder).not.toHaveBeenCalled();
    vi.advanceTimersByTime(300);
    expect(collection.toggleFolder).toHaveBeenCalledWith("f1");
  });

  it("drags a playlist row onto another playlist, and refuses it onto itself", async () => {
    const { playlistActions, nav } = setup();
    dragOnto(row("Theirs"), row("Mine"));
    await tick();
    expect(dragDrop.active).toEqual({
      kind: "collection",
      target: { kind: "playlist", id: "theirs", title: "Theirs" },
      cover: null,
    });
    expect(dragDrop.verdict).toEqual({ ok: true, hint: "Add to Mine" });
    pointer("pointerup", window, 30, 30);
    expect(playlistActions.addTracks).toHaveBeenCalledTimes(1);
    // The click after the drag does not open the row it started on.
    row("Theirs").click();
    expect(nav.goto).not.toHaveBeenCalled();
    vi.runOnlyPendingTimers();

    dragOnto(row("Mine"), row("Mine"));
    expect(dragDrop.verdict).toEqual({ ok: false, reason: "Already in this playlist" });
  });

  it("a cancelled drag adds nothing", () => {
    const { source, playlistActions } = setup();
    dragOnto(source, row("Mine"));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    pointer("pointerup", window, 30, 30);
    expect(playlistActions.addTracks).not.toHaveBeenCalled();
  });
});
