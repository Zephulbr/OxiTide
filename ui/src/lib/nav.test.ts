import { describe, expect, it } from "vitest";
import type { PageItem } from "./api";
import { createNav, routeForPageItem } from "./nav.svelte";

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
