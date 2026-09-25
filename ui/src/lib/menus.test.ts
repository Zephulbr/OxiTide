import { describe, expect, it, vi } from "vitest";
import type { Playlist, Track } from "./api";
import type { Favorites } from "./favorites.svelte";
import {
  libraryPlaylistMenuItems,
  queueMenuItems,
  trackMenuItems,
  type TrackMenuContext,
} from "./menus";

function track(id: number): Track {
  return {
    id,
    title: `T${id}`,
    duration: 1,
    artists: [],
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  };
}

function playlist(): Playlist {
  return {
    uuid: "p",
    title: "Mix",
    description: null,
    numberOfTracks: 0,
    duration: 0,
    image: null,
    owned: true,
  };
}

function favorites(has: boolean): Favorites {
  return {
    hasTrack: () => has,
    toggleTrack: vi.fn(),
  } as unknown as Favorites;
}

function playlistCtx(overrides: { busy?: boolean; disabled?: boolean; canMove?: boolean } = {}) {
  return {
    onRemove: vi.fn(),
    onMove: vi.fn(),
    busy: false,
    disabled: false,
    ...overrides,
  };
}

function baseCtx(overrides: Partial<TrackMenuContext> = {}): TrackMenuContext {
  return {
    track: track(1),
    index: 0,
    trackCount: 1,
    favorites: favorites(false),
    play: vi.fn(),
    addToQueue: vi.fn(),
    addToPlaylist: vi.fn(),
    ...overrides,
  };
}

describe("trackMenuItems", () => {
  it("lists the four base actions", () => {
    expect(trackMenuItems(baseCtx()).map((i) => i.id)).toEqual([
      "play",
      "add-queue",
      "add-playlist",
      "favorite",
    ]);
  });

  it("labels favorite according to state", () => {
    expect(trackMenuItems(baseCtx({ favorites: favorites(true) }))[3].label).toBe("Unfavorite");
    expect(trackMenuItems(baseCtx({ favorites: favorites(false) }))[3].label).toBe("Favorite");
  });

  it("invokes each callback once", () => {
    const ctx = baseCtx();
    for (const item of trackMenuItems(ctx)) item.run();

    expect(vi.mocked(ctx.play)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.addToQueue)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.addToPlaylist)).toHaveBeenCalledTimes(1);
  });

  it("appends playlist editing when the context is editable", () => {
    const items = trackMenuItems(
      baseCtx({ index: 1, trackCount: 3, playlist: playlistCtx() }),
    );

    expect(items.map((i) => i.id)).toEqual([
      "play",
      "add-queue",
      "add-playlist",
      "favorite",
      "move-up",
      "move-down",
      "remove",
    ]);
  });

  it("disables move up at the first row and move down at the last", () => {
    const first = trackMenuItems(baseCtx({ index: 0, trackCount: 3, playlist: playlistCtx() }));
    const last = trackMenuItems(baseCtx({ index: 2, trackCount: 3, playlist: playlistCtx() }));

    expect(first.find((i) => i.id === "move-up")?.disabled).toBe(true);
    expect(first.find((i) => i.id === "move-down")?.disabled).toBe(false);
    expect(last.find((i) => i.id === "move-down")?.disabled).toBe(true);
    expect(last.find((i) => i.id === "move-up")?.disabled).toBe(false);
  });

  it("omits editing items while busy or disabled", () => {
    const busy = trackMenuItems(baseCtx({ playlist: playlistCtx({ busy: true }) }));
    const disabled = trackMenuItems(baseCtx({ playlist: playlistCtx({ disabled: true }) }));

    expect(busy.map((i) => i.id)).not.toContain("remove");
    expect(disabled.map((i) => i.id)).not.toContain("remove");
  });

  it("keeps remove and favorite but omits the moves while canMove is false", () => {
    const items = trackMenuItems(
      baseCtx({ index: 1, trackCount: 3, playlist: playlistCtx({ canMove: false }) }),
    );

    expect(items.map((i) => i.id)).toEqual([
      "play",
      "add-queue",
      "add-playlist",
      "favorite",
      "remove",
    ]);
  });

  it("calls onMove and onRemove with the row index", () => {
    const playlistActions = playlistCtx();
    const items = trackMenuItems(
      baseCtx({ index: 2, trackCount: 3, playlist: playlistActions }),
    );

    items.find((i) => i.id === "move-up")?.run();
    items.find((i) => i.id === "move-down")?.run();
    items.find((i) => i.id === "remove")?.run();

    expect(playlistActions.onMove).toHaveBeenCalledWith(2, -1);
    expect(playlistActions.onMove).toHaveBeenCalledWith(2, 1);
    expect(playlistActions.onRemove).toHaveBeenCalledWith(2);
  });
});

describe("queueMenuItems", () => {
  it("lists play now and remove and invokes them once", () => {
    const playNow = vi.fn();
    const remove = vi.fn();

    const items = queueMenuItems({ index: 0, playNow, remove });

    expect(items.map((i) => i.id)).toEqual(["play-now", "remove"]);
    for (const item of items) item.run();
    expect(playNow).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledTimes(1);
  });
});

describe("libraryPlaylistMenuItems", () => {
  it("lists open, rename, and a danger delete and invokes them once", () => {
    const open = vi.fn();
    const rename = vi.fn();
    const remove = vi.fn();

    const items = libraryPlaylistMenuItems({ playlist: playlist(), open, rename, remove });

    expect(items.map((i) => i.id)).toEqual(["open", "rename", "delete"]);
    expect(items.find((i) => i.id === "delete")?.danger).toBe(true);
    for (const item of items) item.run();
    expect(open).toHaveBeenCalledTimes(1);
    expect(rename).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledTimes(1);
  });
});
