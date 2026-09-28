import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { api, type CollectionEntry } from "./api";
import { errorMessage } from "./errors";
import type { Toasts } from "./toasts.svelte";

export interface CollectionTree {
  readonly loading: boolean;
  readonly error: string;
  readonly root: readonly CollectionEntry[];
  children(folderId: string): readonly CollectionEntry[] | null;
  isExpanded(folderId: string): boolean;
  isLoading(folderId: string): boolean;
  ensureRoot(): Promise<void>;
  toggleFolder(folderId: string): Promise<void>;
  refresh(): Promise<void>;
  reset(): void;
}

/// The sidebar's root folders/playlists plus lazily fetched folder children.
/// Views call `refresh()` after a successful mutation; the root and any
/// expanded folders are then reloaded.
export function createCollectionTree(toasts: Toasts): CollectionTree {
  let root = $state<CollectionEntry[]>([]);
  let loading = $state(false);
  let error = $state("");
  const childrenMap = new SvelteMap<string, CollectionEntry[]>();
  const expanded = new SvelteSet<string>();
  const loadingFolders = new SvelteSet<string>();

  async function loadRoot(): Promise<void> {
    loading = true;
    error = "";
    try {
      root = await api.getCollectionItems("root", "NAME");
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  async function loadChildren(folderId: string): Promise<void> {
    if (loadingFolders.has(folderId)) return;
    loadingFolders.add(folderId);
    try {
      childrenMap.set(folderId, await api.getCollectionItems(folderId, "NAME"));
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      loadingFolders.delete(folderId);
    }
  }

  async function ensureRoot(): Promise<void> {
    if (root.length > 0 || loading) return;
    await loadRoot();
  }

  async function toggleFolder(folderId: string): Promise<void> {
    if (expanded.has(folderId)) {
      expanded.delete(folderId);
      return;
    }
    expanded.add(folderId);
    if (!childrenMap.has(folderId)) await loadChildren(folderId);
  }

  async function refresh(): Promise<void> {
    childrenMap.clear();
    await loadRoot();
    for (const folderId of [...expanded]) await loadChildren(folderId);
  }

  function reset(): void {
    root = [];
    error = "";
    loading = false;
    childrenMap.clear();
    expanded.clear();
    loadingFolders.clear();
  }

  return {
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    get root() {
      return root;
    },
    children: (folderId) => childrenMap.get(folderId) ?? null,
    isExpanded: (folderId) => expanded.has(folderId),
    isLoading: (folderId) => loadingFolders.has(folderId),
    ensureRoot,
    toggleFolder,
    refresh,
    reset,
  };
}
