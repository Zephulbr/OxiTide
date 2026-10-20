import { describe, expect, it, vi } from "vitest";
import type { PageItem } from "./api";
import { NAV_KEY, createNav, routeForPageItem } from "./nav.svelte";

type NavStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

function fakeStorage(seed?: string): NavStorage {
  const map = new Map<string, string>();
  if (seed !== undefined) map.set(NAV_KEY, seed);
  return {
    getItem: (key) => map.get(key) ?? null,
    setItem: (key, value) => void map.set(key, value),
    removeItem: (key) => void map.delete(key),
  };
}

describe("nav history", () => {
  it("starts at home with no history", () => {
    const nav = createNav();

    expect(nav.route).toEqual({ name: "home" });
    expect(nav.canBack).toBe(false);
    expect(nav.canForward).toBe(false);
  });

  it("ignores a goto to the current route", () => {
    const nav = createNav();

    nav.goto({ name: "home" });

    expect(nav.canBack).toBe(false);
  });

  it("ignores a goto to the same album id", () => {
    const nav = createNav();
    nav.goto({ name: "album", id: 1 });

    nav.goto({ name: "album", id: 1 });

    expect(nav.canBack).toBe(true);
    nav.back();
    expect(nav.route).toEqual({ name: "home" });
  });

  it("treats the same name with a different id as a new entry", () => {
    const nav = createNav();
    nav.goto({ name: "album", id: 1 });
    nav.goto({ name: "album", id: 2 });

    nav.back();

    expect(nav.route).toEqual({ name: "album", id: 1 });
  });

  it("truncates forward entries on a new navigation", () => {
    const nav = createNav();
    nav.goto({ name: "explore" });
    nav.goto({ name: "feed" });
    nav.back();
    expect(nav.canForward).toBe(true);

    nav.goto({ name: "settings" });

    expect(nav.canForward).toBe(false);
    expect(nav.route).toEqual({ name: "settings" });
  });

  it("gives each pushed entry its own identity, revisited by back/forward", () => {
    const nav = createNav();
    const home = nav.entry;
    nav.goto({ name: "explore" });
    const explore = nav.entry;
    expect(explore).not.toBe(home);

    nav.back();
    expect(nav.entry).toBe(home);
    nav.forward();
    expect(nav.entry).toBe(explore);

    // A new navigation after going back is a new entry, even for the same route.
    nav.back();
    nav.goto({ name: "explore" });
    expect(nav.entry).not.toBe(explore);
    expect(nav.entry).not.toBe(home);
  });

  it("keeps the entry identity across replace", () => {
    const nav = createNav();
    nav.goto({ name: "artist-all", id: 1, tab: "albums" });
    const entry = nav.entry;
    nav.replace({ name: "artist-all", id: 1, tab: "eps" });
    expect(nav.entry).toBe(entry);
  });

  it("keeps a search in the explore route: Back from a result returns to it", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "explore" });
    nav.replace({ name: "explore", q: "ado", kind: "artists" });
    nav.goto({ name: "artist", id: 7 });

    nav.back();
    expect(nav.route).toEqual({ name: "explore", q: "ado", kind: "artists" });
    // The plain Explore is a different place from the search.
    nav.goto({ name: "explore" });
    expect(nav.route).toEqual({ name: "explore" });
    // And it survives a restart, with an unknown kind dropped.
    expect(createNav(storage).route).toEqual({ name: "explore" });
    const seeded = fakeStorage(
      JSON.stringify({ history: [{ name: "explore", q: "ado", kind: "videos" }], index: 0 }),
    );
    expect(createNav(seeded).route).toEqual({ name: "explore", q: "ado" });
  });

  it("clamps back and forward at the ends", () => {
    const nav = createNav();
    nav.goto({ name: "explore" });

    nav.back();
    nav.back();
    expect(nav.route).toEqual({ name: "home" });

    nav.forward();
    nav.forward();
    expect(nav.route).toEqual({ name: "explore" });
  });

  it("keeps distinct folder ids in history", () => {
    const nav = createNav();
    nav.goto({ name: "folder", id: "a" });
    nav.goto({ name: "folder", id: "b" });

    nav.back();

    expect(nav.route).toEqual({ name: "folder", id: "a" });
  });

  it("keeps the queue toggle out of history", () => {
    const nav = createNav();

    nav.toggleQueue();

    expect(nav.queueOpen).toBe(true);
    expect(nav.canBack).toBe(false);
  });

  it("toggles now playing independently of the route", () => {
    const nav = createNav();
    expect(nav.nowPlayingOpen).toBe(false);
    nav.toggleNowPlaying();
    expect(nav.nowPlayingOpen).toBe(true);
    nav.goto({ name: "settings" });
    expect(nav.nowPlayingOpen).toBe(true);
    nav.closeNowPlaying();
    expect(nav.nowPlayingOpen).toBe(false);
  });
});

describe("nav persistence", () => {
  it("restores the last route and history in a new nav", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "playlist", id: "p1" });
    nav.goto({ name: "folder", id: "f1", label: "MISC" });

    const restored = createNav(storage);

    expect(restored.route).toEqual({ name: "folder", id: "f1", label: "MISC" });
    expect(restored.canBack).toBe(true);
    restored.back();
    expect(restored.route).toEqual({ name: "playlist", id: "p1" });
  });

  it("persists back and forward", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "album", id: 7 });
    nav.goto({ name: "playlist", id: "p1" });
    nav.back();

    const restored = createNav(storage);

    expect(restored.route).toEqual({ name: "album", id: 7 });
    expect(restored.canForward).toBe(true);
  });

  it("starts at home when the stored value is corrupt", () => {
    expect(createNav(fakeStorage("not json")).route).toEqual({ name: "home" });
    expect(createNav(fakeStorage('{"history":[{"name":"nope"}],"index":0}')).route).toEqual({
      name: "home",
    });
    expect(createNav(fakeStorage('{"history":[],"index":0}')).route).toEqual({ name: "home" });
    expect(createNav(fakeStorage('{"history":[{"name":"home"}],"index":5}')).route).toEqual({
      name: "home",
    });
  });

  it("starts at home when a stored route id is degenerate", () => {
    const cases = [
      '{"history":[{"name":"playlist","id":""}],"index":0}',
      '{"history":[{"name":"mix","id":""}],"index":0}',
      '{"history":[{"name":"folder","id":""}],"index":0}',
      '{"history":[{"name":"album","id":0}],"index":0}',
      '{"history":[{"name":"album","id":-3}],"index":0}',
      '{"history":[{"name":"artist","id":0}],"index":0}',
      '{"history":[{"name":"album","id":"7"}],"index":0}',
      '{"history":[{"name":"playlist","id":7}],"index":0}',
    ];
    for (const raw of cases) {
      expect(createNav(fakeStorage(raw)).route, raw).toEqual({ name: "home" });
    }
  });

  it("reset clears the stored route", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "playlist", id: "p1" });

    nav.reset();

    expect(nav.route).toEqual({ name: "home" });
    expect(nav.canBack).toBe(false);
    expect(createNav(storage).route).toEqual({ name: "home" });
  });

  it("reset removes the stored key", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "playlist", id: "p1" });

    nav.reset();

    expect(storage.getItem(NAV_KEY)).toBe(null);
  });

  it("does not write when navigation does not change", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    const write = vi.spyOn(storage, "setItem");

    nav.goto({ name: "home" });
    nav.back();
    nav.forward();

    expect(write).not.toHaveBeenCalled();
  });

  it("keeps navigating when storage fails", () => {
    const storage: NavStorage = {
      getItem: () => null,
      setItem: () => {
        throw new Error("quota exceeded");
      },
      removeItem: () => {
        throw new Error("unavailable");
      },
    };
    const nav = createNav(storage);

    expect(() => nav.goto({ name: "playlist", id: "p1" })).not.toThrow();
    expect(nav.route).toEqual({ name: "playlist", id: "p1" });
    expect(() => nav.reset()).not.toThrow();
    expect(nav.route).toEqual({ name: "home" });
  });
});

function pageItem(item_type: string, id: string): PageItem {
  return { id, title: "Item", item_type };
}

describe("routeForPageItem", () => {
  it("routes playlist-ish items with a non-numeric id", () => {
    expect(routeForPageItem(pageItem("PLAYLIST_LIST", "8f5d0e3a-abc"))).toEqual({
      name: "playlist",
      id: "8f5d0e3a-abc",
    });
    expect(routeForPageItem(pageItem("EDITORIAL", "story-1"))).toEqual({
      name: "playlist",
      id: "story-1",
    });
  });

  it("routes album items with a numeric id", () => {
    expect(routeForPageItem(pageItem("ALBUM", "12345"))).toEqual({
      name: "album",
      id: 12345,
    });
  });

  it("routes mix items with a non-empty id", () => {
    expect(routeForPageItem(pageItem("MIX", "mix-1"))).toEqual({ name: "mix", id: "mix-1" });
    expect(routeForPageItem(pageItem("MIX_LIST", "mix-2"))).toEqual({ name: "mix", id: "mix-2" });
  });

  it("leaves tracks, videos, and empty ids inert", () => {
    expect(routeForPageItem(pageItem("TRACK_LIST", "55"))).toBeNull();
    expect(routeForPageItem(pageItem("VIDEO", "77"))).toBeNull();
    expect(routeForPageItem(pageItem("", "1"))).toBeNull();
    expect(routeForPageItem(pageItem("ALBUM", ""))).toBeNull();
  });

  it("rejects type and id mismatches", () => {
    expect(routeForPageItem(pageItem("PLAYLIST_LIST", "55"))).toBeNull();
    expect(routeForPageItem(pageItem("ALBUM", "not-a-number"))).toBeNull();
  });
});

describe("section routes", () => {
  it("round-trips a section route through storage", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "section", source: "home", title: "Suggested new albums" });

    const restored = createNav(storage);

    expect(restored.route).toEqual({
      name: "section",
      source: "home",
      title: "Suggested new albums",
    });
  });

  it("treats two sections as distinct history entries", () => {
    const nav = createNav();
    nav.goto({ name: "section", source: "home", title: "A" });
    nav.goto({ name: "section", source: "home", title: "B" });

    expect(nav.canBack).toBe(true);
    nav.back();
    expect(nav.route).toEqual({ name: "section", source: "home", title: "A" });
  });

  it("rejects stored sections with an unknown source or empty title", () => {
    const cases = [
      '{"history":[{"name":"section","source":"mixes","title":"A"}],"index":0}',
      '{"history":[{"name":"section","source":"home","title":""}],"index":0}',
      '{"history":[{"name":"section","source":"home"}],"index":0}',
    ];
    for (const raw of cases) {
      expect(createNav(fakeStorage(raw)).route, raw).toEqual({ name: "home" });
    }
  });
});

describe("page routes", () => {
  const link = (api_path: string, title = "Jazz"): PageItem => ({
    id: "",
    title,
    item_type: "PAGE_LINKS_CLOUD",
    api_path,
  });

  it("routes page links by their api path", () => {
    expect(routeForPageItem(link("pages/genre_jazz"))).toEqual({
      name: "page",
      path: "pages/genre_jazz",
      title: "Jazz",
    });
    expect(routeForPageItem(link("pages/genre_jazz", ""))?.name).toBe("page");
    expect(routeForPageItem(link("pages/../x"))).toBeNull();
    expect(routeForPageItem(link("https://x/pages/y"))).toBeNull();
  });

  it("round-trips page routes and page-sourced sections, dedupes by path", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "page", path: "pages/genre_jazz", title: "Jazz" });
    nav.goto({ name: "page", path: "pages/genre_jazz", title: "Renamed" });
    nav.goto({ name: "section", source: "pages/genre_jazz", title: "Essentials" });
    const restored = createNav(storage);
    expect(restored.route).toEqual({ name: "section", source: "pages/genre_jazz", title: "Essentials" });
    restored.back();
    expect(restored.route).toEqual({ name: "page", path: "pages/genre_jazz", title: "Jazz" });
  });

  it("rejects stored pages with a bad path", () => {
    const cases = [
      '{"history":[{"name":"page","path":"albums/1","title":"A"}],"index":0}',
      '{"history":[{"name":"page","path":"pages/x?y","title":"A"}],"index":0}',
      '{"history":[{"name":"page","path":"pages/x","title":""}],"index":0}',
      '{"history":[{"name":"section","source":"pages/../x","title":"A"}],"index":0}',
    ];
    for (const raw of cases) {
      expect(createNav(fakeStorage(raw)).route, raw).toEqual({ name: "home" });
    }
  });
});

describe("routeForPageItem artists", () => {
  it("routes artist items with a numeric id", () => {
    expect(routeForPageItem(pageItem("ARTIST", "42"))).toEqual({ name: "artist", id: 42 });
    expect(routeForPageItem(pageItem("ARTIST_LIST", "42"))).toEqual({ name: "artist", id: 42 });
    expect(routeForPageItem(pageItem("ARTIST", "nope"))).toBeNull();
  });
});

describe("artist catalog routes", () => {
  it("round-trips an artist-all route through storage", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "artist-all", id: 945, tab: "playlists" });

    const restored = createNav(storage);

    expect(restored.route).toEqual({ name: "artist-all", id: 945, tab: "playlists" });
  });

  it("treats different tabs of the same artist as distinct entries", () => {
    const nav = createNav();
    nav.goto({ name: "artist-all", id: 945, tab: "albums" });
    nav.goto({ name: "artist-all", id: 945, tab: "eps" });

    expect(nav.canBack).toBe(true);
    nav.back();
    expect(nav.route).toEqual({ name: "artist-all", id: 945, tab: "albums" });
  });

  it("ignores a goto to the same artist and tab", () => {
    const nav = createNav();
    nav.goto({ name: "artist-all", id: 945, tab: "tracks" });
    nav.goto({ name: "artist-all", id: 945, tab: "tracks" });

    nav.back();
    expect(nav.route).toEqual({ name: "home" });
  });

  it("rejects stored artist-all routes with an unknown tab", () => {
    const raw = '{"history":[{"name":"artist-all","id":945,"tab":"videos"}],"index":0}';
    expect(createNav(fakeStorage(raw)).route).toEqual({ name: "home" });
  });

  it("replace swaps the current entry without growing history", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "artist-all", id: 945, tab: "albums" });

    nav.replace({ name: "artist-all", id: 945, tab: "eps" });

    expect(nav.route).toEqual({ name: "artist-all", id: 945, tab: "eps" });

    const restored = createNav(storage);
    expect(restored.route).toEqual({ name: "artist-all", id: 945, tab: "eps" });

    expect(nav.canBack).toBe(true);
    nav.back();
    expect(nav.route).toEqual({ name: "home" });
  });
});

describe("history routes", () => {
  it("round-trips a history tab through storage", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "history", tab: "stats" });

    const restored = createNav(storage);

    expect(restored.route).toEqual({ name: "history", tab: "stats" });
  });

  it("treats different tabs of history as distinct entries", () => {
    const nav = createNav();
    nav.goto({ name: "history", tab: "recent" });
    nav.goto({ name: "history", tab: "stats" });

    expect(nav.canBack).toBe(true);
    nav.back();
    expect(nav.route).toEqual({ name: "history", tab: "recent" });
  });

  it("ignores a goto to the same history tab", () => {
    const nav = createNav();
    nav.goto({ name: "history", tab: "recent" });
    nav.goto({ name: "history", tab: "recent" });

    nav.back();
    expect(nav.route).toEqual({ name: "home" });
  });

  it("rejects stored history routes with an unknown tab", () => {
    const raw = '{"history":[{"name":"history","tab":"charts"}],"index":0}';
    expect(createNav(fakeStorage(raw)).route).toEqual({ name: "home" });
  });
});
