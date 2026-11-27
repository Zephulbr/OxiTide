// @vitest-environment jsdom
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import type { Nav } from "../lib/nav.svelte";
import { clearViewCache } from "../lib/viewCache";
import CollectionAlbums from "./CollectionAlbums.svelte";

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  const album = (id: number, title: string, artist: string) => ({
    id,
    title,
    artists: [{ id, name: artist }],
    numberOfTracks: 10,
    duration: 0,
  });
  const api = new Proxy(
    {},
    {
      get: (_t, name) => {
        if (name === "getFavoriteAlbums")
          return async () => [
            album(1, "OK Computer", "Radiohead"),
            album(2, "Lemonade", "Beyoncé"),
            album(3, "Kid A", "Radiohead"),
          ];
        return async () => [];
      },
    },
  );
  return { ...actual, api };
});

beforeAll(() => {
  // jsdom has no Web Animations; the grid's `animate:flip` needs both.
  Element.prototype.animate ??= function () {
    return { cancel() {}, finish() {}, finished: Promise.resolve(), playState: "finished" } as unknown as Animation;
  };
  Element.prototype.getAnimations ??= () => [];
});

beforeEach(clearViewCache);
afterEach(cleanup);

const nav = () => ({ goto: vi.fn(), route: { name: "albums" } }) as unknown as Nav;

describe("Albums: the filter box (issue #14)", () => {
  it("narrows the grid by title or artist", async () => {
    render(CollectionAlbums, { nav: nav() });
    await screen.findByText("Lemonade");
    const box = screen.getByLabelText("Search albums");

    await fireEvent.input(box, { target: { value: "radiohead" } });
    expect(screen.getByText("OK Computer")).toBeTruthy();
    expect(screen.getByText("Kid A")).toBeTruthy();
    expect(screen.queryByText("Lemonade")).toBeNull();

    await fireEvent.input(box, { target: { value: "beyonce" } });
    expect(screen.getByText("Lemonade")).toBeTruthy();
    expect(screen.queryByText("Kid A")).toBeNull();
  });

  it("says so when nothing matches, and the count stays the collection's", async () => {
    render(CollectionAlbums, { nav: nav() });
    await screen.findByText("Lemonade");
    await fireEvent.input(screen.getByLabelText("Search albums"), { target: { value: "zzz" } });
    expect(screen.getByText("No albums match “zzz”.")).toBeTruthy();
    expect(screen.getByText("3 albums")).toBeTruthy();
  });
});
