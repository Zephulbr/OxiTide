<script lang="ts">
  import type { CollectionEntry } from "../api";
  import type { CollectionActions } from "../collectionActions.svelte";
  import type { CollectionTree } from "../collection.svelte";
  import { menuFor } from "../menu.svelte";
  import { folderMeta } from "../folderMeta.svelte";
  import { collectionFolderMenuItems, collectionPlaylistMenuItems } from "../menus";
  import type { Nav } from "../nav.svelte";
  import { playlistRecency } from "../playlistRecency.svelte";
  import { filterCollectionEntries, sortCollectionEntries } from "../playlistSort";
  import { playlistView } from "../playlistView.svelte";
  import { playSource, playlistRowState } from "../playSource.svelte";
  import Icon from "./Icon.svelte";
  import Loading from "./Loading.svelte";
  import NowPlayingMark from "./NowPlayingMark.svelte";
  import PlaylistCover from "./PlaylistCover.svelte";
  import PlaylistTree from "./PlaylistTree.svelte";

  let {
    entries,
    collection,
    nav,
    actions,
    rail = false,
    playing = false,
    parentId = "root",
  }: {
    entries: readonly CollectionEntry[];
    collection: CollectionTree;
    nav: Nav;
    actions: CollectionActions;
    rail?: boolean;
    /// Playback is running: the playing source's mark spins.
    playing?: boolean;
    /// The folder these entries sit in ("root" at the top), for Move.
    parentId?: string;
  } = $props();

  const recency = playlistRecency();
  const view = playlistView();
  const colors = folderMeta();

  // Folders first, then the selected sort/filter; reactive so pressing Play or
  // changing the header menu reorders the tree immediately.
  const rows = $derived(
    sortCollectionEntries(filterCollectionEntries(entries, view.prefs.filter), {
      sort: view.prefs.sort,
      dir: view.prefs.dir,
      rank: (uuid) => recency.rank(uuid),
    }),
  );

  function openFolder(id: string, label: string) {
    nav.goto({ name: "folder", id, label });
  }

  function openPlaylist(id: string) {
    nav.goto({ name: "playlist", id });
  }
</script>

{#each rows as entry (entry.kind === "folder" ? `f:${entry.id}` : `p:${entry.uuid}`)}
  {#if entry.kind === "folder"}
    <button
      class="row"
      class:rail
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
      <span class="thumb folder" class:tinted={colors.hue(entry.id) !== null} style:--hue={colors.hue(entry.id) ?? undefined}><Icon name="folder" size={rail ? 20 : 18} /></span>
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
            {playing}
            parentId={entry.id}
          />
        </div>
      {/if}
    {/if}
  {:else}
    {@const isActive = nav.route.name === "playlist" && nav.route.id === entry.uuid}
    {@const rowState = playlistRowState(playSource.playlistId === entry.uuid, isActive)}
    <button
      class="row"
      class:rail
      class:active={rowState === "active" || rowState === "source-active"}
      class:source={rowState === "source" || rowState === "source-active"}
      aria-current={isActive ? "page" : undefined}
      onclick={() => openPlaylist(entry.uuid)}
      use:menuFor={() =>
        collectionPlaylistMenuItems({
          playlist: entry,
          open: () => openPlaylist(entry.uuid),
          rename: () => actions.openRenamePlaylist(entry),
          remove: () => actions.openRemove(entry),
          move: () => actions.openMove(entry, parentId),
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
        {#if rowState === "source" || rowState === "source-active"}
          <span class="playing-mark" aria-hidden="true"><NowPlayingMark {playing} size={16} /></span>
        {/if}
      </span>
      {#if !rail}
        <span class="text">
          <span class="label">{entry.title}</span>
          <span class="count tnum">{entry.numberOfTracks} {entry.numberOfTracks === 1 ? "track" : "tracks"}</span>
        </span>
      {/if}
    </button>
  {/if}
{/each}

<style>
  .row {
    /* Full tree rows carry a two-line label (title over track count), so
       the cover grows to its height; the collapsed rail keeps the list
       thumb size its centring is built on. */
    --tree-thumb: calc(var(--thumb-size) + 14px);
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    padding: 5px 9px;
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
  /* The playback source: accent label + play glyph on the cover. It gets the
     row background only when it is also the open playlist (.active), so the
     open highlight is never lost to the playing marker; a source-only row
     still takes the hover surface tint. */
  .row.source {
    color: var(--accent);
  }
  .thumb {
    position: relative;
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: var(--tree-thumb);
    height: var(--tree-thumb);
    border-radius: 6px;
    overflow: hidden;
  }
  .row.rail {
    --tree-thumb: calc(var(--thumb-size) + 2px);
    padding: 6px 9px;
  }
  /* A user-picked folder colour (lib/folderMeta), on the icon only. */
  .thumb.folder.tinted {
    color: oklch(var(--folder-l) var(--folder-c) var(--hue));
  }
  .playing-mark {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg) 55%, transparent);
  }
  .text {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    line-height: 1.3;
  }
  /* Names lead (full text colour, heavier, a touch larger) so they stand
     apart from the muted track counts; the open / playing rows keep their
     accent. */
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
    font-size: 14.5px;
    font-weight: 550;
  }
  .row.active .label,
  .row.source .label {
    color: inherit;
  }
  .count {
    font-size: 12px;
    color: var(--text-muted);
    opacity: 0.8;
    white-space: nowrap;
  }
  .nested {
    margin-left: 14px;
  }
  /* Under the labels: row padding + cover + gap. */
  .tree-note {
    padding-left: calc(9px + var(--thumb-size) + 14px + 11px);
  }
</style>
