// @vitest-environment jsdom
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import type { CollectionActions } from "../lib/collectionActions.svelte";
import type { Nav } from "../lib/nav.svelte";
import type { PlaylistActions } from "../lib/playlists.svelte";
import { clearViewCache } from "../lib/viewCache";
import Playlists from "./Playlists.svelte";

const tree = vi.hoisted(() => ({
  calls: [] as string[],
  failing: new Set<string>(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  const playlist = (uuid: string, title: string, numberOfTracks = 1) => ({
    kind: "playlist",
    uuid,
    title,
    numberOfTracks,
    owned: true,
  });
  const folder = (id: string, name: string) => ({ kind: "folder", id, name, totalNumberOfItems: 1 });
  const folders: Record<string, unknown[]> = {
    root: [playlist("p1", "Morning Coffee"), folder("gym", "Gym")],
    gym: [playlist("p2", "Leg Day", 12), folder("cardio", "Cardio")],
    cardio: [playlist("p3", "Running Coffee", 3)],
  };
  const api = new Proxy(
    {},
    {
      get: (_t, name) => {
        if (name === "getCollectionItems")
          return async (folderId: string) => {
            tree.calls.push(folderId);
            if (tree.failing.has(folderId)) throw new Error("offline");
            return folders[folderId] ?? [];
          };
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

beforeEach(() => {
  clearViewCache();
  tree.calls = [];
  tree.failing.clear();
});
afterEach(cleanup);

function props() {
  return {
    nav: { goto: vi.fn(), route: { name: "playlists" } } as unknown as Nav,
    playlistActions: { revision: 0, openCreate: vi.fn() } as unknown as PlaylistActions,
    actions: { revision: 0, openCreateFolder: vi.fn() } as unknown as CollectionActions,
  };
}

describe("Playlists: the filter box (issue #14)", () => {
  it("finds playlists inside folders from All playlists, naming their folder", async () => {
    render(Playlists, props());
    await screen.findByText("Morning Coffee");
    expect(screen.queryByText("Leg Day")).toBeNull();

    await fireEvent.input(screen.getByLabelText("Search playlists"), { target: { value: "coffee" } });
    // This level answers at once; the folders' matches follow.
    expect(screen.getByText("Morning Coffee")).toBeTruthy();
    await screen.findByText("Running Coffee");
    expect(screen.getByText("Cardio · 3 tracks")).toBeTruthy();
    expect(screen.queryByText("Gym")).toBeNull();
    // The Gym card's cover mosaic already loaded Gym; the walk reuses it.
    expect([...tree.calls].sort()).toEqual(["cardio", "gym", "root"]);
  });

  it("matches folder names too, and keeps the walk for later keystrokes", async () => {
    render(Playlists, props());
    await screen.findByText("Morning Coffee");
    const box = screen.getByLabelText("Search playlists");
    await fireEvent.input(box, { target: { value: "leg" } });
    await screen.findByText("Gym · 12 tracks");
    await fireEvent.input(box, { target: { value: "cardio" } });
    expect(screen.getByText("Cardio")).toBeTruthy();
    expect(tree.calls.filter((id) => id === "gym")).toHaveLength(1);
  });

  it("says when nothing matches", async () => {
    render(Playlists, props());
    await screen.findByText("Morning Coffee");
    await fireEvent.input(screen.getByLabelText("Search playlists"), { target: { value: "zzz" } });
    await screen.findByText("No playlists or folders match “zzz”.");
  });

  it("says a match may be missing when a folder fails to load", async () => {
    tree.failing.add("gym");
    render(Playlists, props());
    await screen.findByText("Morning Coffee");
    await fireEvent.input(screen.getByLabelText("Search playlists"), { target: { value: "coffee" } });
    await screen.findByText(/Some folders couldn't be searched/);
    expect(screen.getByText("Morning Coffee")).toBeTruthy();
    expect(screen.queryByText("Running Coffee")).toBeNull();
  });

  it("walks below the shown folders only once the box is used", async () => {
    render(Playlists, props());
    await screen.findByText("Morning Coffee");
    await new Promise((r) => setTimeout(r, 20));
    expect(tree.calls).not.toContain("cardio");
    await fireEvent.focus(screen.getByLabelText("Search playlists"));
    await waitFor(() => expect(tree.calls).toContain("cardio"));
  });
});
