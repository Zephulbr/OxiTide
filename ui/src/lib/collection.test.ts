import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CollectionEntry } from "./api";
import { api } from "./api";
import { createCollectionTree } from "./collection.svelte";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({ api: { getCollectionItems: vi.fn() } }));

function playlist(uuid: string): CollectionEntry {
  return {
    kind: "playlist",
    uuid,
    title: uuid,
    description: null,
    numberOfTracks: 0,
    duration: 0,
    image: null,
    owned: true,
  };
}

function folder(id: string): CollectionEntry {
  return { kind: "folder", id, name: id, totalNumberOfItems: 1 };
}

function fakeToasts(): Toasts {
  return {
    get toasts() {
      return [];
    },
    show(_message: string, _options?: { severity?: ToastSeverity }) {
      return 1;
    },
    dismiss() {},
  };
}

describe("collection tree store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads the root once", async () => {
    vi.mocked(api.getCollectionItems).mockResolvedValue([playlist("p1")]);
    const tree = createCollectionTree(fakeToasts());

    await tree.ensureRoot();
    await tree.ensureRoot();

    expect(api.getCollectionItems).toHaveBeenCalledTimes(1);
    expect(api.getCollectionItems).toHaveBeenCalledWith("root", "NAME");
    expect(tree.root).toHaveLength(1);
  });

  it("loads folder children lazily and collapses without refetching", async () => {
    vi.mocked(api.getCollectionItems).mockImplementation(async (folderId) =>
      folderId === "root" ? [folder("f1")] : [playlist("p2")],
    );
    const tree = createCollectionTree(fakeToasts());
    await tree.ensureRoot();

    await tree.toggleFolder("f1");
    expect(tree.isExpanded("f1")).toBe(true);
    expect(tree.children("f1")).toHaveLength(1);

    await tree.toggleFolder("f1");
    await tree.toggleFolder("f1");

    expect(api.getCollectionItems).toHaveBeenCalledTimes(2);
  });

  it("refresh reloads the root and expanded folders", async () => {
    vi.mocked(api.getCollectionItems).mockImplementation(async (folderId) =>
      folderId === "root" ? [folder("f1")] : [playlist("p2")],
    );
    const tree = createCollectionTree(fakeToasts());
    await tree.ensureRoot();
    await tree.toggleFolder("f1");

    await tree.refresh();

    expect(api.getCollectionItems).toHaveBeenCalledTimes(4);
    expect(tree.children("f1")).toHaveLength(1);
  });

  it("surfaces load failures without clearing the tree", async () => {
    vi.mocked(api.getCollectionItems).mockRejectedValue("offline");
    const tree = createCollectionTree(fakeToasts());

    await tree.ensureRoot();

    expect(tree.error).toBe("offline");
    expect(tree.loading).toBe(false);
  });

  it("reset empties the tree and expansion", async () => {
    vi.mocked(api.getCollectionItems).mockResolvedValue([folder("f1")]);
    const tree = createCollectionTree(fakeToasts());
    await tree.ensureRoot();
    await tree.toggleFolder("f1");

    tree.reset();

    expect(tree.root).toHaveLength(0);
    expect(tree.isExpanded("f1")).toBe(false);
    expect(tree.children("f1")).toBeNull();
  });
});
