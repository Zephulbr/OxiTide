// @vitest-environment jsdom
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import type { Nav } from "../lib/nav.svelte";
import type { Toasts } from "../lib/toasts.svelte";
import { fakePlayer } from "../lib/test/fakes";
import { clearViewCache } from "../lib/viewCache";
import Artist from "./Artist.svelte";

const own = vi.hoisted(() => ({
  artistId: null as number | null,
  userId: null as number | null,
  empty: false,
  opened: [] as string[],
  updates: [] as unknown[],
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  const album = (id: number) => ({ id, title: `A${id}`, cover: null, artists: [{ id: 1, name: "X" }], type: "ALBUM" });
  const api = new Proxy(
    {},
    {
      get: (_t, name) => {
        if (name === "getArtist")
          return async (id: number) => ({ id, name: "Artist", picture: null, mixes: {}, user_id: own.userId });
        if (name === "getUserPublicPlaylists")
          return async (userId: number) => [
            { uuid: "p-1", title: `Public of ${userId}`, numberOfTracks: 2, duration: 0, squareImage: null, owned: false },
          ];
        if (name === "getEditableProfile")
          return async () => ({
            artistId: 1, name: "Artist", handle: "artist", bio: "Old", bioId: "bio-1", links: [], hasPicture: false,
          });
        if (name === "updateMyProfile")
          return async (edit: unknown, bio: unknown) => {
            own.updates.push({ edit, bio });
            return { artistId: 1, name: "Renamed", handle: "artist", bio: "Old", bioId: "bio-1", links: [], hasPicture: false };
          };
        if (name === "getArtistProfile") return async () => null;
        if (name === "getArtistTopTracks") return async () => [];
        if (name === "getArtistAlbums") return async () => (own.empty ? [] : [album(1), album(2)]);
        if (name === "getSimilarArtists") return async () => [{ id: 9, name: "Sim", picture: null }];
        if (name === "getMyProfile") return async () => ({ userId: 42, artistId: own.artistId });
        if (name === "openExternal")
          return async (url: string) => {
            own.opened.push(url);
          };
        return async () => [];
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
});

beforeEach(() => {
  clearViewCache();
  own.artistId = null;
  own.userId = null;
  own.empty = false;
  own.opened = [];
  own.updates = [];
});
afterEach(cleanup);

const props = () => ({
  id: 1,
  nav: { goto: vi.fn() } as unknown as Nav,
  player: fakePlayer(),
  favorites: { hasArtist: () => false, toggleArtist: vi.fn(), hasTrack: () => false } as never,
  playlistActions: { openPicker: vi.fn() } as never,
  toasts: { show: vi.fn() } as unknown as Toasts,
});

describe("Artist page cache", () => {
  // Regression: a cached remount applied synchronously inside the mount effect,
  // whose tracked reads made it re-run on its own writes (endless loop).
  it("mounts, then remounts from cache without looping", async () => {
    render(Artist, props());
    await waitFor(() => expect(screen.getByText("Albums")).toBeTruthy(), { timeout: 2000 });
    cleanup();
    render(Artist, props());
    await waitFor(() => expect(screen.getByText("Albums")).toBeTruthy(), { timeout: 2000 });
  }, 10000);
});

describe("Artist page: the account's own profile", () => {
  it("is any artist's page for someone else's artist", async () => {
    own.artistId = 99;
    render(Artist, props());
    await screen.findByText("Albums");
    expect(screen.getByText("Artist", { selector: ".kicker" })).toBeTruthy();
    expect(screen.getByLabelText("Favorite artist")).toBeTruthy();
    expect(screen.queryByText("Edit profile")).toBeNull();
  });

  it("swaps Favorite for Edit profile, which edits the profile in place", async () => {
    own.artistId = 1;
    render(Artist, props());
    await screen.findByText("Your profile", { selector: ".kicker" });
    expect(screen.queryByLabelText("Favorite artist")).toBeNull();
    await fireEvent.click(screen.getByText("Edit profile"));
    const name = await screen.findByLabelText("Name");
    await fireEvent.input(name, { target: { value: "Renamed" } });
    await fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(own.updates).toEqual([{ edit: { name: "Renamed" }, bio: null }]));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(own.opened).toEqual([]);
  });

  it("lists a profile's public playlists", async () => {
    own.userId = 9;
    render(Artist, props());
    await screen.findByText("Public playlists");
    expect(screen.getByText("Public of 9")).toBeTruthy();
  });

  it("has no public playlists shelf for an artist without an owner", async () => {
    render(Artist, props());
    await screen.findByText("Similar artists");
    expect(screen.queryByText("Public playlists")).toBeNull();
  });

  it("says an empty profile is empty", async () => {
    own.artistId = 1;
    own.empty = true;
    render(Artist, props());
    await screen.findByText(/^Your profile is empty/);
  });
});
