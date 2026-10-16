// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/svelte";
import type { Nav } from "../lib/nav.svelte";
import type { Toasts } from "../lib/toasts.svelte";
import { fakePlayer } from "../lib/test/fakes";
import Artist from "./Artist.svelte";

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  const album = (id: number) => ({ id, title: `A${id}`, cover: null, artists: [{ id: 1, name: "X" }], type: "ALBUM" });
  const api = new Proxy(
    {},
    {
      get: (_t, name) => {
        if (name === "getArtist") return async (id: number) => ({ id, name: "Artist", picture: null, mixes: {} });
        if (name === "getArtistProfile") return async () => null;
        if (name === "getArtistTopTracks") return async () => [];
        if (name === "getArtistAlbums") return async () => [album(1), album(2)];
        if (name === "getSimilarArtists") return async () => [{ id: 9, name: "Sim", picture: null }];
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
