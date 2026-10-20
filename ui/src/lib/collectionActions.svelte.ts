import { api, type CollectionEntry, type CollectionFolder, type Playlist } from "./api";
import { errorMessage } from "./errors";
import { folderMeta, type FolderColorId } from "./folderMeta.svelte";
import type { CollectionTree } from "./collection.svelte";
import type { Toasts } from "./toasts.svelte";
import { invalidate } from "./viewCache";

export type RenameTarget =
  | { kind: "playlist"; playlist: Playlist }
  | { kind: "folder"; folder: CollectionFolder };

export interface FolderOption {
  folder: CollectionFolder;
  depth: number;
}

/// A folder's local details, edited alongside its name.
export interface FolderDetails {
  description: string;
  color: FolderColorId | null;
}

export interface CreateFolderTarget {
  parentId: string;
}

export interface CollectionActions {
  readonly renameTarget: RenameTarget | null;
  readonly removeTarget: CollectionEntry | null;
  readonly moveTarget: Playlist | null;
  /// The folder the move target sits in now ("root": in no folder).
  readonly moveFrom: string;
  readonly createFolderTarget: CreateFolderTarget | null;
  readonly busy: boolean;
  readonly revision: number;
  readonly folderOptions: readonly FolderOption[];
  readonly foldersLoading: boolean;
  openRenamePlaylist(playlist: Playlist): void;
  openRenameFolder(folder: CollectionFolder): void;
  openRemove(entry: CollectionEntry): void;
  openMove(playlist: Playlist, fromFolderId?: string): void;
  openCreateFolder(parentId: string): void;
  close(): void;
  /// Folders also take a description and colour (kept on this device,
  /// folderMeta).
  rename(value: string, folder?: FolderDetails): Promise<void>;
  confirmRemove(): Promise<void>;
  move(folderId: string): Promise<void>;
  createFolder(name: string, color?: FolderColorId | null): Promise<void>;
  reset(): void;
}

const MAX_FOLDER_DEPTH = 5;

/// Playlist/folder mutations shared by the sidebar tree and the Playlists
/// page. Successful writes refresh the `CollectionTree` and bump `revision`
/// so the page can reload; failures only toast and keep dialogs open.
export function createCollectionActions(
  collection: CollectionTree,
  toasts: Toasts,
): CollectionActions {
  let renameTarget = $state<RenameTarget | null>(null);
  let removeTarget = $state<CollectionEntry | null>(null);
  let moveTarget = $state<Playlist | null>(null);
  let moveFrom = $state("root");
  let createFolderTarget = $state<CreateFolderTarget | null>(null);
  let busy = $state(false);
  let revision = $state(0);
  let folderOptions = $state<FolderOption[]>([]);
  let foldersLoading = $state(false);

  function close() {
    renameTarget = null;
    removeTarget = null;
    moveTarget = null;
    createFolderTarget = null;
  }

  function reset() {
    close();
    busy = false;
    folderOptions = [];
    foldersLoading = false;
  }

  async function collectFolders(parentId: string, depth: number): Promise<FolderOption[]> {
    if (depth > MAX_FOLDER_DEPTH) return [];
    const entries = await api.getCollectionItems(parentId, "NAME");
    const folders = entries
      .filter(
        (item): item is { kind: "folder" } & CollectionFolder => item.kind === "folder",
      )
      .map(({ kind: _kind, ...folder }) => folder);
    const result: FolderOption[] = [];
    for (const folder of folders) {
      result.push({ folder, depth });
      result.push(...(await collectFolders(folder.id, depth + 1)));
    }
    return result;
  }

  async function loadFolders() {
    foldersLoading = true;
    try {
      folderOptions = await collectFolders("root", 0);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      foldersLoading = false;
    }
  }

  function openRenamePlaylist(playlist: Playlist) {
    renameTarget = { kind: "playlist", playlist };
  }

  function openRenameFolder(folder: CollectionFolder) {
    renameTarget = { kind: "folder", folder };
  }

  function openRemove(entry: CollectionEntry) {
    removeTarget = entry;
  }

  function openMove(playlist: Playlist, fromFolderId = "root") {
    moveTarget = playlist;
    moveFrom = fromFolderId;
    folderOptions = [];
    void loadFolders();
  }

  function openCreateFolder(parentId: string) {
    createFolderTarget = { parentId };
  }

  async function rename(value: string, details?: FolderDetails) {
    const target = renameTarget;
    const name = value.trim();
    if (!target || name.length === 0 || busy) return;
    busy = true;
    try {
      if (target.kind === "playlist") {
        await api.renamePlaylist(target.playlist.uuid, name, target.playlist.description ?? "");
        invalidate(`playlist:${target.playlist.uuid}`);
        toasts.show("Playlist renamed", { severity: "success" });
      } else {
        // Only a changed name goes to TIDAL; the description is local.
        const renamed = name !== target.folder.name;
        if (renamed) await api.renameFolder(target.folder.id, name);
        if (details) {
          folderMeta().setDescription(target.folder.id, details.description);
          folderMeta().setColor(target.folder.id, details.color);
        }
        toasts.show(renamed ? "Folder renamed" : "Folder saved", { severity: "success" });
      }
      revision += 1;
      close();
      await collection.refresh();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  async function confirmRemove() {
    const target = removeTarget;
    if (!target || busy) return;
    busy = true;
    try {
      if (target.kind === "folder") {
        await api.removeCollectionEntry("folder", target.id);
        toasts.show(`Removed ${target.name}`, { severity: "success" });
      } else if (target.owned) {
        await api.deletePlaylist(target.uuid);
        invalidate(`playlist:${target.uuid}`);
        toasts.show(`Deleted ${target.title}`, { severity: "success" });
      } else {
        await api.removeCollectionEntry("playlist", target.uuid);
        toasts.show(`Removed ${target.title}`, { severity: "success" });
      }
      revision += 1;
      close();
      await collection.refresh();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  async function createFolder(value: string, color: FolderColorId | null = null) {
    const target = createFolderTarget;
    const name = value.trim();
    if (!target || name.length === 0 || busy) return;
    busy = true;
    try {
      const created = await api.createFolder(name, target.parentId);
      if (color && created?.id) folderMeta().setColor(created.id, color);
      toasts.show(`Created ${name}`, { severity: "success" });
      revision += 1;
      close();
      await collection.refresh();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  async function move(folderId: string) {
    const target = moveTarget;
    if (!target || busy) return;
    busy = true;
    try {
      await api.movePlaylistsToFolder([target.uuid], folderId);
      toasts.show(`Moved ${target.title}`, { severity: "success" });
      revision += 1;
      close();
      await collection.refresh();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  return {
    get renameTarget() {
      return renameTarget;
    },
    get removeTarget() {
      return removeTarget;
    },
    get moveTarget() {
      return moveTarget;
    },
    get moveFrom() {
      return moveFrom;
    },
    get createFolderTarget() {
      return createFolderTarget;
    },
    get busy() {
      return busy;
    },
    get revision() {
      return revision;
    },
    get folderOptions() {
      return folderOptions;
    },
    get foldersLoading() {
      return foldersLoading;
    },
    openRenamePlaylist,
    openRenameFolder,
    openRemove,
    openMove,
    openCreateFolder,
    close,
    rename,
    confirmRemove,
    move,
    createFolder,
    reset,
  };
}
