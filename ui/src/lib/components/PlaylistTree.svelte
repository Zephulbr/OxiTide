<script lang="ts">
  import type { CollectionEntry } from "../api";
  import type { CollectionActions } from "../collectionActions.svelte";
  import type { CollectionTree } from "../collection.svelte";
  import { menuFor } from "../menu.svelte";
  import { collectionFolderMenuItems, collectionPlaylistMenuItems } from "../menus";
  import type { Nav } from "../nav.svelte";
  import { playSource, playlistRowState } from "../playSource.svelte";
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
    nav.goto({ name: "folder", id, label });
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
      aria-current={nav.route.name === "folder" && nav.route.id === entry.id ? "page" : undefined}
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
      <span class="thumb folder"><Icon name="folder" size={rail ? 20 : 18} /></span>
      {#if !rail}
        <span class="label">{entry.name}</span>
        <Icon name={collection.isExpanded(entry.id) ? "chevron-down" : "chevron-right"} size={16} />
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
    {@const isActive = nav.route.name === "playlist" && nav.route.id === entry.uuid}
    {@const rowState = playlistRowState(playSource.playlistId === entry.uuid, isActive)}
    <button
      class="row"
      class:active={rowState === "active"}
      class:source={rowState === "source"}
      aria-current={isActive ? "page" : undefined}
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
        {#if rowState === "source"}
          <span class="playing-mark" aria-hidden="true"><Icon name="play" size={12} /></span>
        {/if}
      </span>
      {#if !rail}<span class="label">{entry.title}</span>{/if}
    </button>
  {/if}
{/each}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    padding: 6px 9px;
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
  /* The playback source: accent label + play glyph on the cover, no row
     background (spec/sidebar amendment). Hover keeps just its surface tint. */
  .row.source {
    color: var(--accent);
  }
  .thumb {
    position: relative;
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: calc(var(--thumb-size) + 2px);
    height: calc(var(--thumb-size) + 2px);
    border-radius: 6px;
    overflow: hidden;
  }
  .playing-mark {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.45);
    color: var(--text);
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nested {
    margin-left: 14px;
  }
  .tree-note {
    padding-left: 52px;
  }
</style>
