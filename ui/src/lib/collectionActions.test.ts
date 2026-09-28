import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CollectionEntry, CollectionFolder, Playlist } from "./api";
import { api } from "./api";
import { createCollectionActions } from "./collectionActions.svelte";
import type { CollectionTree } from "./collection.svelte";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({
  api: {
    getCollectionItems: vi.fn(),
    renamePlaylist: vi.fn(),
    renameFolder: vi.fn(),
    deletePlaylist: vi.fn(),
    removeCollectionEntry: vi.fn(),
    movePlaylistsToFolder: vi.fn(),
  },
}));

function playlist(uuid: string, owned = true): Playlist {
  return {
    uuid,
    title: `P${uuid}`,
    description: null,
    numberOfTracks: 1,
    duration: 0,
    image: null,
    owned,
  };
}

function entry(uuid: string, owned = true): CollectionEntry {
  return { kind: "playlist", ...playlist(uuid, owned) };
}

function folder(id: string): CollectionFolder {
  return { id, name: `F${id}`, totalNumberOfItems: 1 };
}

function fakeToasts() {
  const calls: { message: string; severity: ToastSeverity }[] = [];
  const toasts: Toasts = {
    get toasts() {
      return [];
    },
    show(message, options) {
      calls.push({ message, severity: options?.severity ?? "info" });
      return calls.length;
    },
    dismiss() {},
  };
  return { toasts, calls };
}

function fakeTree() {
  const refresh = vi.fn().mockResolvedValue(undefined);
  return { tree: { refresh } as unknown as CollectionTree, refresh };
}

describe("collection actions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renames a playlist, bumps revision, refreshes, and closes", async () => {
    vi.mocked(api.renamePlaylist).mockResolvedValue(undefined);
    const { tree, refresh } = fakeTree();
    const { toasts } = fakeToasts();
    const actions = createCollectionActions(tree, toasts);

    actions.openRenamePlaylist(playlist("p1"));
    await actions.rename("  New name  ");

    expect(api.renamePlaylist).toHaveBeenCalledWith("p1", "New name", "");
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(actions.revision).toBe(1);
    expect(actions.renameTarget).toBeNull();
  });

  it("keeps the rename dialog open and does not refresh when the write fails", async () => {
    vi.mocked(api.renameFolder).mockRejectedValue("forbidden");
    const { tree, refresh } = fakeTree();
    const { toasts, calls } = fakeToasts();
    const actions = createCollectionActions(tree, toasts);

    actions.openRenameFolder(folder("f1"));
    await actions.rename("Nope");

    expect(refresh).not.toHaveBeenCalled();
    expect(actions.revision).toBe(0);
    expect(actions.renameTarget).not.toBeNull();
    expect(calls[0]?.message).toBe("You can only edit your own playlists");
  });

  it("deletes owned playlists but removes liked ones from the collection", async () => {
    vi.mocked(api.deletePlaylist).mockResolvedValue(undefined);
    vi.mocked(api.removeCollectionEntry).mockResolvedValue(undefined);
    const { toasts } = fakeToasts();
    const actions = createCollectionActions(fakeTree().tree, toasts);

    actions.openRemove(entry("owned", true));
    await actions.confirmRemove();
    actions.openRemove(entry("liked", false));
    await actions.confirmRemove();
    actions.openRemove({ kind: "folder", ...folder("f1") });
    await actions.confirmRemove();

    expect(api.deletePlaylist).toHaveBeenCalledWith("owned");
    expect(api.removeCollectionEntry).toHaveBeenCalledWith("playlist", "liked");
    expect(api.removeCollectionEntry).toHaveBeenCalledWith("folder", "f1");
  });

  it("moves a playlist into a folder", async () => {
    vi.mocked(api.movePlaylistsToFolder).mockResolvedValue(undefined);
    const { tree, refresh } = fakeTree();
    const actions = createCollectionActions(tree, fakeToasts().toasts);

    actions.openMove(playlist("p1"));
    await actions.move("f2");

    expect(api.movePlaylistsToFolder).toHaveBeenCalledWith(["p1"], "f2");
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(actions.moveTarget).toBeNull();
  });

  it("lists nested folders with depth for the move picker", async () => {
    vi.mocked(api.getCollectionItems).mockImplementation(async (folderId) => {
      if (folderId === "root") return [{ kind: "folder", ...folder("a") }];
      if (folderId === "a") return [{ kind: "folder", ...folder("b") }];
      return [];
    });
    const actions = createCollectionActions(fakeTree().tree, fakeToasts().toasts);

    actions.openMove(playlist("p1"));
    await vi.waitFor(() => expect(actions.foldersLoading).toBe(false));

    expect(actions.folderOptions).toEqual([
      { folder: folder("a"), depth: 0 },
      { folder: folder("b"), depth: 1 },
    ]);
  });
});
