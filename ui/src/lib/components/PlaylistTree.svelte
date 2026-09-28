<script lang="ts">
  import type { CollectionEntry } from "../api";
  import type { CollectionActions } from "../collectionActions.svelte";
  import type { CollectionTree } from "../collection.svelte";
  import { menuFor } from "../menu.svelte";
  import { collectionFolderMenuItems, collectionPlaylistMenuItems } from "../menus";
  import type { Nav } from "../nav.svelte";
  import Icon from "./Icon.svelte";
  import Loading from "./Loading.svelte";
  import PlaylistCover from "./PlaylistCover.svelte";
  import PlaylistTree from "./PlaylistTree.svelte";

  let {
    entries,
    collection,
    nav,
    actions,
    rail = false,
  }: {
    entries: readonly CollectionEntry[];
    collection: CollectionTree;
    nav: Nav;
    actions: CollectionActions;
    rail?: boolean;
  } = $props();

  function openFolder(id: string, label: string) {
    nav.goto({ name: "folder", id });
  }

  function openPlaylist(id: string) {
    nav.goto({ name: "playlist", id });
  }
</script>

{#each entries as entry (entry.kind === "folder" ? `f:${entry.id}` : `p:${entry.uuid}`)}
  {#if entry.kind === "folder"}
    <button
      class="row"
      class:active={nav.route.name === "folder" && nav.route.id === entry.id}
      onclick={() => (rail ? nav.goto({ name: "playlists" }) : void collection.toggleFolder(entry.id))}
      use:menuFor={() =>
        collectionFolderMenuItems({
          folder: entry,
          open: () => openFolder(entry.id, entry.name),
          rename: () => actions.openRenameFolder(entry),
          remove: () => actions.openRemove(entry),
        })}
      title={entry.name}
    >
      <span class="thumb folder"><Icon name="folder" size={rail ? 20 : 16} /></span>
      {#if !rail}
        <span class="label">{entry.name}</span>
        <Icon name={collection.isExpanded(entry.id) ? "chevron-down" : "chevron-right"} size={14} />
      {/if}
    </button>
    {#if !rail && collection.isExpanded(entry.id)}
      {#if collection.isLoading(entry.id)}
        <div class="tree-note"><Loading /></div>
      {:else if collection.children(entry.id)?.length === 0}
        <p class="muted tree-note">No playlists</p>
      {:else if collection.children(entry.id) !== null}
        <div class="nested">
          <PlaylistTree
            entries={collection.children(entry.id) ?? []}
            {collection}
            {nav}
            {actions}
            {rail}
          />
        </div>
      {/if}
    {/if}
  {:else}
    <button
      class="row"
      class:active={nav.route.name === "playlist" && nav.route.id === entry.uuid}
      onclick={() => openPlaylist(entry.uuid)}
      use:menuFor={() =>
        collectionPlaylistMenuItems({
          playlist: entry,
          open: () => openPlaylist(entry.uuid),
          rename: () => actions.openRenamePlaylist(entry),
          remove: () => actions.openRemove(entry),
          move: () => actions.openMove(entry),
        })}
      title={entry.title}
    >
      <span class="thumb">
        <PlaylistCover
          owned={entry.owned}
          playlist={entry.uuid}
          image={entry.squareImage}
          size={80}
          radius={6}
          alt=""
        />
      </span>
      {#if !rail}<span class="label">{entry.title}</span>{/if}
    </button>
  {/if}
{/each}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 5px 8px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    text-align: left;
  }
  .row:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .row.active {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
  }
  .thumb {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    overflow: hidden;
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nested {
    margin-left: 12px;
  }
  .tree-note {
    padding-left: 44px;
  }
</style>
