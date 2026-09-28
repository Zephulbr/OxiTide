<script lang="ts">
  import { untrack } from "svelte";
  import { api, type CollectionEntry } from "../lib/api";
  import { sortEntries, type CollectionSort } from "../lib/collectionSort";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import Modal from "../lib/components/Modal.svelte";
  import PlaylistCover from "../lib/components/PlaylistCover.svelte";
  import type { CollectionActions } from "../lib/collectionActions.svelte";
  import type { CollectionTree } from "../lib/collection.svelte";
  import { errorMessage } from "../lib/errors";
  import { menuFor } from "../lib/menu.svelte";
  import { collectionFolderMenuItems, collectionPlaylistMenuItems } from "../lib/menus";
  import type { Nav } from "../lib/nav.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let {
    nav,
    playlistActions,
    toasts,
    collection,
    actions,
    folderId = null,
    folderLabel = "",
  }: {
    nav: Nav;
    playlistActions: PlaylistActions;
    toasts: Toasts;
    collection: CollectionTree;
    actions: CollectionActions;
    folderId?: string | null;
    folderLabel?: string;
  } = $props();

  let entries = $state<CollectionEntry[]>([]);
  let loading = $state(true);
  let error = $state("");
  let sortMode = $state<CollectionSort>("date");
  let creatingFolder = $state(false);
  let folderName = $state("");
  let savingFolder = $state(false);
  let actionsSeen = untrack(() => actions.revision);
  let createSeen = untrack(() => playlistActions.revision);

  const rows = $derived(sortEntries(entries, sortMode));

  async function load() {
    loading = true;
    error = "";
    try {
      entries = await api.getCollectionItems(
        folderId ?? "root",
        sortMode === "name" ? "NAME" : undefined,
      );
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    folderId;
    sortMode;
    void load();
  });

  $effect(() => {
    const revision = actions.revision;
    if (revision === actionsSeen) return;
    actionsSeen = revision;
    untrack(() => void load());
  });

  $effect(() => {
    const revision = playlistActions.revision;
    if (revision === createSeen) return;
    createSeen = revision;
    untrack(() => void load());
  });

  function closeCreateFolder() {
    creatingFolder = false;
    folderName = "";
  }

  async function submitFolder() {
    const name = folderName.trim();
    if (name.length === 0 || savingFolder) return;
    savingFolder = true;
    try {
      await api.createFolder(name, folderId ?? "root");
      toasts.show(`Created ${name}`, { severity: "success" });
      closeCreateFolder();
      await load();
      await collection.refresh();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      savingFolder = false;
    }
  }
</script>

<div class="page-head">
  <h2>{folderId ? folderLabel || "Folder" : "All playlists"}</h2>
  <button class="round" onclick={() => playlistActions.openCreate()} title="New playlist" aria-label="New playlist">
    <Icon name="plus" size={16} />
  </button>
  <button class="round" onclick={() => (creatingFolder = true)} title="New folder" aria-label="New folder">
    <Icon name="folder-plus" size={16} />
  </button>
  <button
    class="round"
    onclick={() => (sortMode = sortMode === "name" ? "date" : "name")}
    title={sortMode === "name" ? "Sort by date added" : "Sort by name"}
    aria-label="Toggle sort"
  >
    <Icon name="arrow-up-down" size={16} />
  </button>
</div>
{#if folderId}
  <p class="crumbs"><button onclick={() => nav.goto({ name: "playlists" })}>All playlists</button></p>
{/if}

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if rows.length === 0}
  <EmptyState message={folderId ? "This folder is empty." : "No playlists yet."} />
{:else}
  <div role="list">
    {#each rows as entry (entry.kind === "folder" ? `f:${entry.id}` : `p:${entry.uuid}`)}
      <div
        class="row"
        role="listitem"
        use:menuFor={() =>
          entry.kind === "folder"
            ? collectionFolderMenuItems({
                folder: entry,
                open: () => nav.goto({ name: "folder", id: entry.id, label: entry.name }),
                rename: () => actions.openRenameFolder(entry),
                remove: () => actions.openRemove(entry),
              })
            : collectionPlaylistMenuItems({
                playlist: entry,
                open: () => nav.goto({ name: "playlist", id: entry.uuid }),
                rename: () => actions.openRenamePlaylist(entry),
                remove: () => actions.openRemove(entry),
                move: () => actions.openMove(entry),
              })}
      >
        {#if entry.kind === "folder"}
          <button
            class="row-main"
            onclick={() => nav.goto({ name: "folder", id: entry.id, label: entry.name })}
          >
            <span class="tile folder"><Icon name="folder" size={20} /></span>
            <span class="text">
              <span class="title">{entry.name}</span>
              <span class="muted sub">
                {entry.totalNumberOfItems}
                {entry.totalNumberOfItems === 1 ? "playlist" : "playlists"}
              </span>
            </span>
          </button>
        {:else}
          <button class="row-main" onclick={() => nav.goto({ name: "playlist", id: entry.uuid })}>
            <span class="tile">
              <PlaylistCover
                owned={entry.owned}
                playlist={entry.uuid}
                image={entry.squareImage}
                size={80}
                radius={6}
                alt=""
              />
            </span>
            <span class="text">
              <span class="title">{entry.title}</span>
              <span class="muted sub">{entry.numberOfTracks} items</span>
            </span>
          </button>
        {/if}
      </div>
    {/each}
  </div>
{/if}

{#if creatingFolder}
  <Modal title="New folder" onclose={closeCreateFolder}>
    <form
      onsubmit={(event) => {
        event.preventDefault();
        void submitFolder();
      }}
    >
      <label>
        Name
        <input bind:value={folderName} aria-label="Folder name" />
      </label>
      <p class="dialog-actions">
        <button type="button" onclick={closeCreateFolder}>Cancel</button>
        <button type="submit" disabled={savingFolder || folderName.trim() === ""}>Create</button>
      </p>
    </form>
  </Modal>
{/if}

<style>
  .page-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .page-head h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
  }
  .round {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .round:hover {
    color: var(--text);
  }
  .crumbs {
    margin: 4px 0 12px;
  }
  .crumbs button {
    color: var(--text-muted);
  }
  .crumbs button:hover {
    color: var(--text);
  }
  .row {
    display: flex;
    align-items: center;
  }
  .row-main {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
    padding: 6px 4px;
    border-radius: var(--radius-control);
    text-align: left;
  }
  .row-main:hover {
    background: var(--surface-2);
  }
  .tile {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 6px;
    overflow: hidden;
    background: var(--surface-2);
    color: var(--text-muted);
  }
  .text {
    display: grid;
    min-width: 0;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11.5px;
  }
  label {
    display: grid;
    gap: 6px;
  }
  input {
    width: 100%;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text);
  }
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
</style>
