// @vitest-environment jsdom
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/svelte";
import type { Component } from "svelte";
import type { Nav } from "../lib/nav.svelte";
import { fakePlayer } from "../lib/test/fakes";
import { clearViewCache } from "../lib/viewCache";
import CollectionAlbums from "./CollectionAlbums.svelte";
import Feed from "./Feed.svelte";
import Mixes from "./Mixes.svelte";
import Playlists from "./Playlists.svelte";
import Profiles from "./Profiles.svelte";

const calls = vi.hoisted(() => ({ count: 0 }));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  const album = { id: 1, title: "Album One", cover: null, artists: [{ id: 2, name: "X" }], type: "ALBUM" };
  const data: Record<string, unknown> = {
    getFeed: [{ id: 5, title: "Release One", subtitle: "X", artists: [], seen: false, releaseType: "ALBUM", artwork: null }],
    getFavoriteAlbums: [album],
    getFavoriteArtists: [{ id: 3, name: "Artist One", picture: null }],
    getFavoriteMixes: [{ id: "m1", title: "Mix One", subTitle: null, image: null }],
    getForYou: [],
    getCollectionItems: [
      { kind: "playlist", uuid: "p1", title: "Playlist One", owned: true, squareImage: null, numberOfTracks: 1 },
    ],
  };
  const api = new Proxy(
    {},
    {
      get: (_t, name: string) => async () => {
        calls.count += 1;
        return data[name] ?? [];
      },
    },
  );
  return { ...actual, api };
});

beforeAll(() => {
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
  Element.prototype.animate ??= function () {
    return { cancel() {}, finish() {}, finished: Promise.resolve(), playState: "finished" } as unknown as Animation;
  };
});

beforeEach(() => clearViewCache());
afterEach(cleanup);

const nav = { goto: vi.fn(), route: { name: "home" } } as unknown as Nav;
const toasts = { show: vi.fn() } as never;
const revisioned = { revision: 0, openCreate: vi.fn(), openCreateFolder: vi.fn() } as never;

const pages: Array<[string, Component<any>, Record<string, unknown>, string]> = [
  ["Feed", Feed as Component<any>, { nav }, "Release One"],
  ["Albums", CollectionAlbums as Component<any>, { nav }, "Album One"],
  ["Profiles", Profiles as Component<any>, { nav }, "Artist One"],
  ["Mixes", Mixes as Component<any>, { nav, player: fakePlayer(), toasts }, "Mix One"],
  ["Playlists", Playlists as Component<any>, { nav, playlistActions: revisioned, actions: revisioned }, "Playlist One"],
];

describe("library pages and the page cache", () => {
  it.each(pages)("%s mounts, then remounts from cache without looping", async (_name, page, props, text) => {
    render(page, props as never);
    await waitFor(() => expect(screen.getByText(text)).toBeTruthy(), { timeout: 2000 });
    cleanup();
    calls.count = 0;
    render(page, props as never);
    // From cache: on screen at once, with one background revalidation at most.
    expect(screen.getByText(text)).toBeTruthy();
    await new Promise((r) => setTimeout(r, 50));
    expect(calls.count).toBeLessThanOrEqual(2);
  });
});
