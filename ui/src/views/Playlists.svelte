<script lang="ts">
  import { untrack } from "svelte";
  import { api, type CollectionEntry } from "../lib/api";
  import { sortEntries, type CollectionSort } from "../lib/collectionSort";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PlaylistCover from "../lib/components/PlaylistCover.svelte";
  import SortSelect from "../lib/components/SortSelect.svelte";
  import type { CollectionActions } from "../lib/collectionActions.svelte";
  import { errorMessage } from "../lib/errors";
  import { menuFor } from "../lib/menu.svelte";
  import { collectionFolderMenuItems, collectionPlaylistMenuItems } from "../lib/menus";
  import type { Nav } from "../lib/nav.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";

  let {
    nav,
    playlistActions,
    actions,
    folderId = null,
    folderLabel = "",
  }: {
    nav: Nav;
    playlistActions: PlaylistActions;
    actions: CollectionActions;
    folderId?: string | null;
    folderLabel?: string;
  } = $props();

  let entries = $state<CollectionEntry[]>([]);
  let loading = $state(true);
  let error = $state("");
  let sortMode = $state<CollectionSort>("date");
  let actionsSeen = untrack(() => actions.revision);
  let createSeen = untrack(() => playlistActions.revision);

  const rows = $derived(sortEntries(entries, sortMode));
  const summary = $derived.by(() => {
    const folders = entries.filter((entry) => entry.kind === "folder").length;
    const playlists = entries.length - folders;
    const parts = [`${playlists} ${playlists === 1 ? "playlist" : "playlists"}`];
    if (folders > 0) parts.push(`${folders} ${folders === 1 ? "folder" : "folders"}`);
    return parts.join(" · ");
  });

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
</script>

{#if folderId}
  <nav class="crumbs" aria-label="Breadcrumb">
    <button onclick={() => nav.goto({ name: "playlists" })}>All playlists</button>
    <Icon name="chevron-right" size={12} />
  </nav>
{/if}
<header class="page-head">
  <h2>{folderId ? folderLabel || "Folder" : "All playlists"}</h2>
  {#if !loading && !error && entries.length > 0}<span class="muted count tnum">{summary}</span>{/if}
  <button class="tool-btn" onclick={() => playlistActions.openCreate()}>
    <Icon name="plus" size={15} />New playlist
  </button>
  <button class="tool-btn" onclick={() => actions.openCreateFolder(folderId ?? "root")}>
    <Icon name="folder-plus" size={15} />New folder
  </button>
  <SortSelect
    value={sortMode}
    options={[
      { value: "date", label: "Recently updated" },
      { value: "name", label: "Name A–Z" },
    ]}
    label="Sort playlists"
    onchange={(value) => (sortMode = value as CollectionSort)}
  />
</header>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if rows.length === 0}
  <EmptyState message={folderId ? "This folder is empty." : "No playlists yet."} />
{:else}
  <div class="grid pane" role="list">
    {#each rows as entry (entry.kind === "folder" ? `f:${entry.id}` : `p:${entry.uuid}`)}
      <div
        class="cell"
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
          <button class="card" onclick={() => nav.goto({ name: "folder", id: entry.id, label: entry.name })}>
            <span class="art folder"><Icon name="folder" size={40} /></span>
            <span class="title">{entry.name}</span>
            <span class="muted sub">
              Folder · {entry.totalNumberOfItems}
              {entry.totalNumberOfItems === 1 ? "playlist" : "playlists"}
            </span>
          </button>
        {:else}
          <button class="card" onclick={() => nav.goto({ name: "playlist", id: entry.uuid })}>
            <span class="art">
              <span class="clip">
                <PlaylistCover
                  owned={entry.owned}
                  playlist={entry.uuid}
                  image={entry.squareImage}
                  size={320}
                  radius={0}
                  alt=""
                />
              </span>
            </span>
            <span class="title">{entry.title}</span>
            <span class="muted sub tnum">
              {entry.numberOfTracks}
              {entry.numberOfTracks === 1 ? "track" : "tracks"}
            </span>
          </button>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .crumbs {
    display: flex;
    align-items: center;
    gap: 4px;
    margin: 0 0 6px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .crumbs button {
    color: var(--text-muted);
  }
  .crumbs button:hover {
    color: var(--text);
  }
  /* Same grid as Albums and Mixes: tiles with the playlist "sheet" behind the
     cover (MediaCard's playlist shape), folders as a sleeve with an icon. */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
  }
  .cell {
    min-width: 0;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    min-width: 0;
    padding: 12px;
    border-radius: var(--radius-card);
    text-align: left;
    color: inherit;
  }
  .card:hover {
    background: var(--surface-2);
  }
  .art {
    position: relative;
    isolation: isolate;
    display: block;
    aspect-ratio: 1;
  }
  .art::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    transform: translate(3px, 3px);
    border: 1px solid var(--line);
    border-radius: var(--radius-card-sharp);
    background: var(--surface-3);
  }
  .clip {
    display: block;
    height: 100%;
    overflow: hidden;
    border-radius: var(--radius-card-sharp);
    background: var(--surface-2);
  }
  .clip :global(img) {
    transition: transform var(--dur-state) var(--ease-enter);
  }
  .card:hover .clip :global(img) {
    transform: scale(1.03);
  }
  .art.folder {
    display: grid;
    place-items: center;
    border-radius: var(--radius-card-sharp);
    background: linear-gradient(135deg, var(--surface-2), var(--surface-3));
    color: var(--text-muted);
  }
  .card:hover .art.folder {
    color: var(--text);
  }
  .title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    margin-top: -6px;
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
