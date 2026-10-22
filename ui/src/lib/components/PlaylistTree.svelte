<script lang="ts">
  import type { CollectionEntry, Playlist } from "../api";
  import type { CollectionActions } from "../collectionActions.svelte";
  import type { CollectionTree } from "../collection.svelte";
  import { dragDrop, dragSource, dropZone, type DropZone } from "../dragDrop.svelte";
  import type { DragPayload } from "../dragPayload";
  import { playlistDropKey, playlistDropZone } from "../playlistDrop";
  import type { PlaylistActions } from "../playlists.svelte";
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
    playlistActions,
  }: {
    entries: readonly CollectionEntry[];
    collection: CollectionTree;
    nav: Nav;
    actions: CollectionActions;
    /// Makes playlist rows drop targets for dragged tracks (drag-to-playlist
    /// spec §4.2); without it the tree takes no drops.
    playlistActions?: PlaylistActions;
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

  /// A playlist row as a drop target; a zone with no verdict when the tree
  /// takes no drops.
  function playlistZone(entry: Playlist): DropZone {
    if (!playlistActions) return { key: playlistDropKey(entry.uuid) };
    return playlistDropZone(entry, playlistActions, () => openPlaylist(entry.uuid));
  }

  /// A collapsed folder springs open under a drag that rests on it (DD-D9).
  const SPRING_MS = 600;
  function folderZone(id: string): DropZone {
    if (rail || !playlistActions) return { key: `folder:${id}` };
    return {
      key: `folder:${id}`,
      spring: {
        ms: SPRING_MS,
        open: () => {
          if (!collection.isExpanded(id)) void collection.toggleFolder(id);
        },
      },
    };
  }

  /// A playlist row drags the playlist itself (onto another playlist or the
  /// queue), like its card on the Playlists page.
  function playlistPayload(entry: Playlist): DragPayload {
    return {
      kind: "collection",
      target: { kind: "playlist", id: entry.uuid, title: entry.title },
      cover: entry.squareImage ?? entry.image ?? null,
    };
  }

  function dropState(uuid: string): "target" | "refused" | null {
    if (dragDrop.targetKey !== playlistDropKey(uuid)) return null;
    return dragDrop.verdict?.ok ? "target" : "refused";
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
      use:dropZone={folderZone(entry.id)}
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
            {playlistActions}
            parentId={entry.id}
          />
        </div>
      {/if}
    {/if}
  {:else}
    {@const isActive = nav.route.name === "playlist" && nav.route.id === entry.uuid}
    {@const rowState = playlistRowState(playSource.playlistId === entry.uuid, isActive)}
    {@const drop = dropState(entry.uuid)}
    {@const adding = dragDrop.pending.has(playlistDropKey(entry.uuid))}
    <button
      class="row"
      class:rail
      class:active={rowState === "active" || rowState === "source-active"}
      class:source={rowState === "source" || rowState === "source-active"}
      class:drop-target={drop === "target"}
      class:drop-refused={drop === "refused"}
      data-drop-refused={drop === "refused" ? "" : undefined}
      aria-busy={adding || undefined}
      aria-current={isActive ? "page" : undefined}
      onclick={() => openPlaylist(entry.uuid)}
      use:dropZone={playlistZone(entry)}
      use:dragSource={() => playlistPayload(entry)}
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
        {#if adding}
          <span class="adding" aria-hidden="true"><span class="disc"></span></span>
        {:else if rowState === "source" || rowState === "source-active"}
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
  /* Drop targets (drag-to-playlist spec §7): an accent ring on the row a drop
     would add to; a refusing row dims. */
  .row.drop-target {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    box-shadow: inset 0 0 0 2px var(--accent);
    color: var(--text);
  }
  .row.drop-refused {
    opacity: 0.55;
  }
  /* An add in flight: a small spinning disc over the cover. */
  .adding {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg) 55%, transparent);
  }
  .adding .disc {
    width: 14px;
    height: 14px;
    border: 2px solid var(--line);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: adding-spin 0.9s linear infinite;
  }
  @keyframes adding-spin {
    to {
      transform: rotate(360deg);
    }
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
