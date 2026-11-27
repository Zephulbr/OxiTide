import { describe, expect, it, vi } from "vitest";
import type { CollectionEntry } from "./api";
import {
  MAX_WALK_DEPTH,
  MAX_WALK_FOLDERS,
  matchCollection,
  walkCollection,
  type FoundEntry,
} from "./collectionSearch";

function folder(id: string, name: string): CollectionEntry {
  return { kind: "folder", id, name, totalNumberOfItems: 0 };
}

function playlist(uuid: string, title: string): CollectionEntry {
  return { kind: "playlist", uuid, title, numberOfTracks: 1 } as CollectionEntry;
}

const names = (found: readonly FoundEntry[]) =>
  found.map(({ entry }) => (entry.kind === "folder" ? entry.name : entry.title));

describe("walkCollection", () => {
  const tree: Record<string, CollectionEntry[]> = {
    gym: [playlist("p2", "Leg Day"), folder("cardio", "Cardio")],
    cardio: [playlist("p3", "Running Mix")],
    chill: [playlist("p4", "Sunday")],
  };
  const root = [playlist("p1", "Favourites"), folder("gym", "Gym"), folder("chill", "Chill")];

  it("finds entries in the start folder and every folder beneath it", async () => {
    const walk = await walkCollection({ id: "root", name: null, entries: root }, async (id) => tree[id]);
    expect(walk.complete).toBe(true);
    expect(names(walk.found)).toEqual(["Favourites", "Gym", "Chill", "Leg Day", "Cardio", "Sunday", "Running Mix"]);
    const running = walk.found.find(({ entry }) => entry.kind === "playlist" && entry.uuid === "p3");
    expect(running).toMatchObject({ parentId: "cardio", parentName: "Cardio" });
    expect(walk.found[0]).toMatchObject({ parentId: "root", parentName: null });
  });

  it("never refetches the start folder or a folder seen twice", async () => {
    const fetchFolder = vi.fn(async (id: string) => (id === "a" ? [folder("a", "A"), folder("root", "Loop")] : []));
    await walkCollection({ id: "root", name: null, entries: [folder("a", "A")] }, fetchFolder);
    expect(fetchFolder.mock.calls.map(([id]) => id)).toEqual(["a"]);
  });

  it("skips a folder that fails and reports the walk incomplete", async () => {
    const walk = await walkCollection({ id: "root", name: null, entries: root }, async (id) => {
      if (id === "gym") throw new Error("offline");
      return tree[id];
    });
    expect(walk.complete).toBe(false);
    expect(names(walk.found)).toEqual(["Favourites", "Gym", "Chill", "Sunday"]);
  });

  it("stops at the depth cap", async () => {
    // Each folder holds the next one: d1 > d2 > … without end.
    const fetchFolder = vi.fn(async (id: string) => {
      const n = Number(id.slice(1)) + 1;
      return [folder(`d${n}`, `Depth ${n}`)];
    });
    const walk = await walkCollection({ id: "root", name: null, entries: [folder("d1", "Depth 1")] }, fetchFolder);
    expect(walk.complete).toBe(false);
    expect(fetchFolder).toHaveBeenCalledTimes(MAX_WALK_DEPTH);
  });

  it("stops at the folder cap", async () => {
    const many = Array.from({ length: MAX_WALK_FOLDERS + 5 }, (_, i) => folder(`f${i}`, `F${i}`));
    const fetchFolder = vi.fn(async () => []);
    const walk = await walkCollection({ id: "root", name: null, entries: many }, fetchFolder);
    expect(walk.complete).toBe(false);
    expect(fetchFolder).toHaveBeenCalledTimes(MAX_WALK_FOLDERS);
  });
});

describe("matchCollection", () => {
  const found: FoundEntry[] = [
    { entry: playlist("p1", "Café Classics"), parentId: "root", parentName: null },
    { entry: folder("gym", "Gym"), parentId: "root", parentName: null },
    { entry: playlist("p2", "Gym Hits"), parentId: "gym", parentName: "Gym" },
    { entry: playlist("p2", "Gym Hits"), parentId: "gym", parentName: "Gym" },
  ];

  it("matches folder and playlist names, folding accents, each once", () => {
    expect(names(matchCollection(found, "gym"))).toEqual(["Gym", "Gym Hits"]);
    expect(names(matchCollection(found, "cafe"))).toEqual(["Café Classics"]);
  });

  it("matches nothing for an empty query", () => {
    expect(matchCollection(found, " ")).toEqual([]);
  });
});
