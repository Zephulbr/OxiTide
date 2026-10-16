import { describe, expect, it, vi } from "vitest";
import type { Playlist, Track } from "./api";
import type { Favorites } from "./favorites.svelte";
import {
  collectionCardMenuItems,
  historyMenuItems,
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

function playlistCtx(overrides: { busy?: boolean; disabled?: boolean } = {}) {
  return {
    onRemove: vi.fn(),
    busy: false,
    disabled: false,
    ...overrides,
  };
}

function baseCtx(overrides: Partial<TrackMenuContext> = {}): TrackMenuContext {
  return {
    track: track(1),
    index: 0,
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
    expect(favorite(true)?.icon).toBe("heart-crack");
    expect(favorite(false)?.label).toBe("Favorite");
    expect(favorite(false)?.icon).toBe("heart");
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
    const items = trackMenuItems(baseCtx({ index: 1, playlist: playlistCtx() }));

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
    expect(items.find((i) => i.id === "remove")?.danger).toBe(true);
  });

  it("gives every track row an icon and groups them with dividers", () => {
    const items = trackMenuItems(baseCtx({ playlist: playlistCtx() }));

    expect(items.every((i) => i.icon)).toBe(true);
    expect(items.filter((i) => i.divider).map((i) => i.id)).toEqual([
      "add-playlist",
      "copy-title",
      "remove",
    ]);
  });

  it("adds a divided navigation group before copy title", () => {
    const goToAlbum = vi.fn();
    const open = vi.fn();
    const items = trackMenuItems(baseCtx({ goToAlbum, goToArtists: [{ name: "A", open }] }));

    expect(items.map((i) => i.id)).toEqual([
      "play",
      "radio",
      "play-next",
      "add-queue",
      "add-playlist",
      "favorite",
      "go-album",
      "go-artist",
      "copy-title",
      "track-info",
    ]);
    expect(items.find((i) => i.id === "go-album")?.divider).toBe(true);
    expect(items.find((i) => i.id === "go-artist")?.label).toBe("Go to artist");
    items.find((i) => i.id === "go-album")?.run();
    items.find((i) => i.id === "go-artist")?.run();
    expect(goToAlbum).toHaveBeenCalledTimes(1);
    expect(open).toHaveBeenCalledTimes(1);
  });

  it("names each artist when there are several, dividing the first entry", () => {
    const items = trackMenuItems(
      baseCtx({ goToArtists: [{ name: "A", open: vi.fn() }, { name: "B", open: vi.fn() }] }),
    );
    const go = items.filter((i) => i.id.startsWith("go-"));
    expect(go.map((i) => i.label)).toEqual(["Go to A", "Go to B"]);
    expect(go.map((i) => i.divider ?? false)).toEqual([true, false]);
  });

  it("omits editing items while busy or disabled", () => {
    const busy = trackMenuItems(baseCtx({ playlist: playlistCtx({ busy: true }) }));
    const disabled = trackMenuItems(baseCtx({ playlist: playlistCtx({ disabled: true }) }));

    expect(busy.map((i) => i.id)).not.toContain("remove");
    expect(disabled.map((i) => i.id)).not.toContain("remove");
  });

  it("calls onRemove with the row index", () => {
    const playlistActions = playlistCtx();
    const items = trackMenuItems(baseCtx({ index: 2, playlist: playlistActions }));

    items.find((i) => i.id === "remove")?.run();

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

  it("adds the go-to group between play actions and copy title", () => {
    const items = queueMenuItems({
      index: 0,
      playNow: vi.fn(),
      copyTitle: vi.fn(),
      remove: vi.fn(),
      goToAlbum: vi.fn(),
      goToArtists: [{ name: "A", open: vi.fn() }],
    });
    expect(items.map((i) => i.id)).toEqual(["play-now", "go-album", "go-artist", "copy-title", "remove"]);
  });

  it("offers play next only when given", () => {
    const playNext = vi.fn();
    const items = queueMenuItems({ index: 3, playNow: vi.fn(), playNext, copyTitle: vi.fn(), remove: vi.fn() });
    expect(items.map((i) => i.id)).toEqual(["play-now", "play-next", "copy-title", "remove"]);
    items[1].run();
    expect(playNext).toHaveBeenCalledTimes(1);
  });
});

describe("historyMenuItems", () => {
  it("lists play then the go-to group", () => {
    const play = vi.fn();
    const items = historyMenuItems({ play, goToAlbum: vi.fn(), goToArtists: [] });
    expect(items.map((i) => i.id)).toEqual(["play", "go-album"]);
    items[0].run();
    expect(play).toHaveBeenCalledTimes(1);
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

describe("collectionCardMenuItems", () => {
  function ctx(extra: Partial<Parameters<typeof collectionCardMenuItems>[0]> = {}) {
    return {
      play: vi.fn(),
      shuffle: vi.fn(),
      playNext: vi.fn(),
      addToQueue: vi.fn(),
      addToPlaylist: vi.fn(),
      copyLink: vi.fn(),
      ...extra,
    };
  }

  it("lists playback, library and link actions", () => {
    const items = collectionCardMenuItems(ctx());
    expect(items.map((item) => item.id)).toEqual([
      "play",
      "shuffle",
      "play-next",
      "add-queue",
      "add-playlist",
      "copy-link",
    ]);
  });

  it("adds the favorite toggle and artist links when given", () => {
    const toggle = vi.fn();
    const open = vi.fn();
    const items = collectionCardMenuItems(
      ctx({ favorite: { active: true, toggle }, goToArtists: [{ name: "A", open }] }),
    );
    const favorite = items.find((item) => item.id === "favorite")!;
    expect(favorite.label).toBe("Unfavorite");
    favorite.run();
    expect(toggle).toHaveBeenCalledTimes(1);
    const artist = items.find((item) => item.id === "go-artist")!;
    expect(artist.divider).toBe(true);
    artist.run();
    expect(open).toHaveBeenCalledTimes(1);
  });

  it("routes each item to its callback", () => {
    const c = ctx();
    for (const item of collectionCardMenuItems(c)) item.run();
    for (const fn of [c.play, c.shuffle, c.playNext, c.addToQueue, c.addToPlaylist, c.copyLink]) {
      expect(fn).toHaveBeenCalledTimes(1);
    }
  });
});
