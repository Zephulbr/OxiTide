import { describe, expect, it, vi } from "vitest";
import type { Playlist, Track } from "./api";
import type { Favorites } from "./favorites.svelte";
import {
  artistMoreItems,
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
    startRadio: vi.fn(),
    playNext: vi.fn(),
    addToQueue: vi.fn(),
    addToPlaylist: vi.fn(),
    copyTitle: vi.fn(),
    trackInfo: vi.fn(),
    ...overrides,
  };
}

describe("trackMenuItems", () => {
  it("lists the eight base actions", () => {
    expect(trackMenuItems(baseCtx()).map((i) => i.id)).toEqual([
      "play",
      "radio",
      "play-next",
      "add-queue",
      "add-playlist",
      "favorite",
      "copy-title",
      "track-info",
    ]);
  });

  it("track info invokes its callback", () => {
    const ctx = { ...baseCtx(), trackInfo: vi.fn() };
    const item = trackMenuItems(ctx).find((entry) => entry.id === "track-info");
    expect(item?.label).toBe("Track info");
    item?.run();
    expect(ctx.trackInfo).toHaveBeenCalledTimes(1);
  });

  it("labels favorite according to state", () => {
    const favorite = (fav: boolean) =>
      trackMenuItems(baseCtx({ favorites: favorites(fav) })).find((i) => i.id === "favorite");
    expect(favorite(true)?.label).toBe("Unfavorite");
    expect(favorite(false)?.label).toBe("Favorite");
  });

  it("disables playback actions for a blocked track but keeps the rest", () => {
    const items = trackMenuItems(baseCtx({ blocked: true }));
    const byId = new Map(items.map((i) => [i.id, i]));
    expect(byId.get("play")?.disabled).toBe(true);
    expect(byId.get("radio")?.disabled).toBe(true);
    expect(byId.get("add-queue")?.disabled).toBe(true);
    expect(byId.get("play-next")?.disabled).toBe(true);
    expect(byId.get("favorite")?.disabled).toBeFalsy();
    expect(byId.get("copy-title")?.disabled).toBeFalsy();
    expect(byId.get("add-playlist")?.disabled).toBeFalsy();
  });

  it("invokes each callback once", () => {
    const ctx = baseCtx();
    for (const item of trackMenuItems(ctx)) item.run();

    expect(vi.mocked(ctx.play)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.startRadio)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.playNext)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.addToQueue)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.addToPlaylist)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.copyTitle)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.trackInfo)).toHaveBeenCalledTimes(1);
  });

  it("appends playlist editing when the context is editable", () => {
    const items = trackMenuItems(
      baseCtx({ index: 1, trackCount: 3, playlist: playlistCtx() }),
    );

    expect(items.map((i) => i.id)).toEqual([
      "play",
      "radio",
      "play-next",
      "add-queue",
      "add-playlist",
      "favorite",
      "copy-title",
      "track-info",
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
      "radio",
      "play-next",
      "add-queue",
      "add-playlist",
      "favorite",
      "copy-title",
      "track-info",
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
  it("lists play now, copy title, and remove and invokes them once", () => {
    const playNow = vi.fn();
    const copyTitle = vi.fn();
    const remove = vi.fn();

    const items = queueMenuItems({ index: 0, playNow, copyTitle, remove });

    expect(items.map((i) => i.id)).toEqual(["play-now", "copy-title", "remove"]);
    for (const item of items) item.run();
    expect(playNow).toHaveBeenCalledTimes(1);
    expect(copyTitle).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledTimes(1);
  });

  it("offers play next only when given", () => {
    const playNext = vi.fn();
    const items = queueMenuItems({ index: 3, playNow: vi.fn(), playNext, copyTitle: vi.fn(), remove: vi.fn() });
    expect(items.map((i) => i.id)).toEqual(["play-now", "play-next", "copy-title", "remove"]);
    items[1].run();
    expect(playNext).toHaveBeenCalledTimes(1);
  });
});

describe("libraryPlaylistMenuItems", () => {
  it("lists open, rename, and a danger delete for an editable playlist", () => {
    const open = vi.fn();
    const rename = vi.fn();
    const remove = vi.fn();

    const items = libraryPlaylistMenuItems({
      playlist: playlist(),
      editable: true,
      open,
      rename,
      remove,
    });

    expect(items.map((i) => i.id)).toEqual(["open", "rename", "delete"]);
    expect(items.find((i) => i.id === "delete")?.danger).toBe(true);
    for (const item of items) item.run();
    expect(open).toHaveBeenCalledTimes(1);
    expect(rename).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledTimes(1);
  });

  it("keeps only Open for a playlist we do not own", () => {
    const open = vi.fn();
    const rename = vi.fn();
    const remove = vi.fn();

    const items = libraryPlaylistMenuItems({
      playlist: playlist(),
      editable: false,
      open,
      rename,
      remove,
    });

    expect(items.map((i) => i.id)).toEqual(["open"]);
    for (const item of items) item.run();
    expect(open).toHaveBeenCalledTimes(1);
    expect(rename).not.toHaveBeenCalled();
    expect(remove).not.toHaveBeenCalled();
  });
});

describe("artistMoreItems", () => {
  it("artistMoreItems adds the mix action only when a mix exists", () => {
    const openMix = vi.fn();
    const without = artistMoreItems({ share: vi.fn(), mixId: null, openMix });
    expect(without.map((item) => item.id)).toEqual(["share"]);

    const withMix = artistMoreItems({ share: vi.fn(), mixId: "m-1", openMix });
    expect(withMix.map((item) => item.id)).toEqual(["share", "artist-mix"]);
    withMix[1].run();
    expect(openMix).toHaveBeenCalledWith("m-1");
  });
});
