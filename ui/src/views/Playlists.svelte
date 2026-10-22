<script lang="ts">
  import { untrack } from "svelte";
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import type { CollectionEntry } from "../lib/api";
  import { appearance } from "../lib/appearance.svelte";
  import { getCardActions } from "../lib/cardActions";
  import { fetchShared, load as loadCached, peek } from "../lib/viewCache";
  import { collectionSource } from "../lib/viewData";
  import { sortEntries, type CollectionSort } from "../lib/collectionSort";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import FolderArt from "../lib/components/FolderArt.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import NowPlayingMark from "../lib/components/NowPlayingMark.svelte";
  import PlaylistCover from "../lib/components/PlaylistCover.svelte";
  import SortSelect from "../lib/components/SortSelect.svelte";
  import type { CollectionActions } from "../lib/collectionActions.svelte";
  import { errorMessage } from "../lib/errors";
  import { folderMeta } from "../lib/folderMeta.svelte";
  import { menuFor } from "../lib/menu.svelte";
  import { collectionFolderMenuItems, collectionPlaylistMenuItems } from "../lib/menus";
  import type { Nav } from "../lib/nav.svelte";
  import { playlistRecency } from "../lib/playlistRecency.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import { playSource } from "../lib/playSource.svelte";

  let {
    nav,
    playlistActions,
    actions,
    folderId = null,
    folderLabel = "",
    playing = false,
  }: {
    nav: Nav;
    playlistActions: PlaylistActions;
    actions: CollectionActions;
    folderId?: string | null;
    folderLabel?: string;
    /// Playback is running: the playing playlist's mark animates.
    playing?: boolean;
  } = $props();

  const cardActions = getCardActions();
  const recency = playlistRecency();
  const colors = folderMeta();

  let sortMode = $state<CollectionSort>("date");
  // svelte-ignore state_referenced_locally
  const sourceFor = () => collectionSource(folderId ?? "root", sortMode === "name" ? "NAME" : undefined);
  // Cached visits render complete on the first frame; a grid that had to
  // load rises in (`.arrive`) instead of snapping in.
  const cached = peek(sourceFor());
  const arrive = cached === undefined;
  let entries = $state<CollectionEntry[]>(cached ?? []);
  let loading = $state(cached === undefined);
  let error = $state("");
  let actionsSeen = untrack(() => actions.revision);
  let createSeen = untrack(() => playlistActions.revision);

  const rows = $derived(sortEntries(entries, sortMode, (uuid) => recency.rank(uuid)));
  const summary = $derived.by(() => {
    const folders = entries.filter((entry) => entry.kind === "folder").length;
    const playlists = entries.length - folders;
    const parts = [`${playlists} ${playlists === 1 ? "playlist" : "playlists"}`];
    if (folders > 0) parts.push(`${folders} ${folders === 1 ? "folder" : "folders"}`);
    return parts.join(" · ");
  });

  /// A sort change or a refresh after an edit (`force`: fetch now, skip the
  /// cache) keeps the current grid on screen until the new list is in; the
  /// cards then glide to their new places (`animate:flip`). Only an empty
  /// page shows the spinner.
  async function load(force = false) {
    const source = sourceFor();
    if (entries.length === 0 && peek(source) === undefined) loading = true;
    error = "";
    try {
      if (force) entries = await fetchShared(source);
      else await loadCached(source, (list) => (entries = list));
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    folderId;
    sortMode;
    untrack(() => void load());
  });

  $effect(() => {
    const revision = actions.revision;
    if (revision === actionsSeen) return;
    actionsSeen = revision;
    untrack(() => void load(true));
  });

  $effect(() => {
    const revision = playlistActions.revision;
    if (revision === createSeen) return;
    createSeen = revision;
    untrack(() => void load(true));
  });

  function openFolder(id: string, label: string) {
    nav.goto({ name: "folder", id, label });
  }

  function openPlaylist(uuid: string) {
    nav.goto({ name: "playlist", id: uuid });
  }

  /// Through the shared card actions (content filters, play source, toasts).
  function play(uuid: string, title: string, shuffle: boolean) {
    void cardActions?.play({ kind: "playlist", id: uuid, title }, shuffle);
  }
</script>

<div class="playlists">
{#if folderId}
  <nav class="crumbs" aria-label="Breadcrumb">
    <button onclick={() => nav.goto({ name: "playlists" })}>All playlists</button>
    <Icon name="chevron-right" size={12} />
  </nav>
{/if}
<header class="page-head">
  <!-- The count sits under the title rather than beside the tools. -->
  <div class="title-block">
    <h2>{folderId ? folderLabel || "Folder" : "All playlists"}</h2>
    {#if !loading && !error && entries.length > 0}<span class="summary tnum">{summary}</span>{/if}
  </div>
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
      { value: "played", label: "Recently played" },
      { value: "name", label: "Name A–Z" },
    ]}
    label="Sort playlists"
    onchange={(value) => (sortMode = value as CollectionSort)}
  />
</header>
{#if folderId && colors.description(folderId)}
  <p class="folder-desc">{colors.description(folderId)}</p>
{/if}

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if rows.length === 0}
  <EmptyState message={folderId ? "This folder is empty." : "No playlists yet."} />
{:else}
  <div class="grid pane" class:arrive role="list">
    {#each rows as entry (entry.kind === "folder" ? `f:${entry.id}` : `p:${entry.uuid}`)}
      <div
        class="cell"
        role="listitem"
        animate:flip={{ duration: appearance.reducedMotion ? 0 : 320, easing: cubicOut }}
        use:menuFor={() =>
          entry.kind === "folder"
            ? collectionFolderMenuItems({
                folder: entry,
                open: () => openFolder(entry.id, entry.name),
                rename: () => actions.openRenameFolder(entry),
                remove: () => actions.openRemove(entry),
              })
            : collectionPlaylistMenuItems({
                playlist: entry,
                open: () => openPlaylist(entry.uuid),
                rename: () => actions.openRenamePlaylist(entry),
                remove: () => actions.openRemove(entry),
                move: () => actions.openMove(entry, folderId ?? "root"),
              })}
      >
        {#if entry.kind === "folder"}
          <button
            class="card"
            title={colors.description(entry.id) || undefined}
            onclick={() => openFolder(entry.id, entry.name)}
          >
            <span class="art"><span class="clip"><FolderArt folderId={entry.id} /></span></span>
            <span class="title">{entry.name}</span>
            <span class="sub">
              Folder · {entry.totalNumberOfItems}
              {entry.totalNumberOfItems === 1 ? "playlist" : "playlists"}
            </span>
          </button>
        {:else}
          {@const source = playSource.playlistId === entry.uuid}
          <!-- The cover and title open the playlist; play and shuffle are
               their own buttons over the cover (MediaCard's split cards). -->
          <div class="card split" class:source>
            <span class="art">
              <button
                type="button"
                class="clip cover-hit"
                aria-label={`Open ${entry.title}`}
                onclick={() => openPlaylist(entry.uuid)}
              >
                <PlaylistCover
                  owned={entry.owned}
                  playlist={entry.uuid}
                  image={entry.squareImage}
                  size={320}
                  radius={0}
                  gapless
                  alt=""
                />
              </button>
              {#if source}
                <span class="playing-mark" aria-hidden="true"><NowPlayingMark {playing} size={34} /></span>
              {/if}
              {#if cardActions}
                <span class="cover-actions">
                  <button
                    type="button"
                    class="act shuffle"
                    aria-label={`Shuffle ${entry.title}`}
                    title="Shuffle"
                    onclick={() => play(entry.uuid, entry.title, true)}
                  ><Icon name="shuffle" size={16} /></button>
                  <button
                    type="button"
                    class="act play"
                    aria-label={`Play ${entry.title}`}
                    title="Play"
                    onclick={() => play(entry.uuid, entry.title, false)}
                  ><Icon name="play" size={18} /></button>
                </span>
              {/if}
            </span>
            <button type="button" class="title name" tabindex="-1" onclick={() => openPlaylist(entry.uuid)}>
              {entry.title}
            </button>
            <span class="sub tnum">
              {entry.numberOfTracks}
              {entry.numberOfTracks === 1 ? "track" : "tracks"}
            </span>
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}
</div>

<style>
  /* The page sits further in from the sidebar and the window edge than the
     shared page padding, and is the size container the grid's column count
     reads. */
  .playlists {
    padding-inline: 28px;
    container: playlists / inline-size;
  }
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
  /* Title over its count, taking the row's free width (as a bare h2 does in
     .page-head), so the tools stay right-aligned. */
  .title-block {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .title-block h2 {
    margin: 0;
  }
  .summary {
    color: color-mix(in srgb, var(--text) 62%, transparent);
    font-size: 14px;
  }
  /* A folder's local description, under the page title. */
  .folder-desc {
    max-width: 72ch;
    margin: -6px 0 18px;
    color: color-mix(in srgb, var(--text) 70%, transparent);
    font-size: 14px;
    line-height: 1.5;
    white-space: pre-line;
  }
  /* Eight tiles across a wide window, fewer as it narrows (tiles stay about
     160px or wider): seamless covers, folders as a mosaic of what they hold.
     Column counts are container queries, so the queue glide never sees a
     late reflow. */
  .grid {
    display: grid;
    grid-template-columns: repeat(8, minmax(0, 1fr));
    gap: 20px 16px;
  }
  @container playlists (width < 1440px) {
    .grid { grid-template-columns: repeat(7, minmax(0, 1fr)); }
  }
  @container playlists (width < 1260px) {
    .grid { grid-template-columns: repeat(6, minmax(0, 1fr)); }
  }
  @container playlists (width < 1080px) {
    .grid { grid-template-columns: repeat(5, minmax(0, 1fr)); }
  }
  @container playlists (width < 900px) {
    .grid { grid-template-columns: repeat(4, minmax(0, 1fr)); }
  }
  @container playlists (width < 720px) {
    .grid { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  }
  @container playlists (width < 540px) {
    .grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
  .cell {
    min-width: 0;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    min-width: 0;
    padding: 10px;
    border-radius: var(--radius-card);
    text-align: left;
    color: inherit;
  }
  .art {
    position: relative;
    isolation: isolate;
    display: block;
    aspect-ratio: 1;
    margin-bottom: 9px;
    /* The whole cover (and its actions) lifts on hover, as on Home's cards. */
    transition: transform var(--dur-state) var(--ease-enter);
  }
  .card:hover .art,
  .card.split:focus-within .art {
    transform: translateY(-3px);
  }
  .clip {
    display: block;
    width: 100%;
    height: 100%;
    padding: 0;
    border: 0;
    overflow: hidden;
    border-radius: var(--radius-card-sharp);
    background: var(--surface-2);
    box-shadow: 0 0 0 1px var(--line), var(--shadow-cover);
    color: inherit;
  }
  .cover-hit {
    cursor: pointer;
  }
  .cover-hit:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .clip :global(img) {
    transition: transform var(--dur-state) var(--ease-enter);
  }
  .card:hover .clip :global(img) {
    transform: scale(1.03);
  }

  /* The playing playlist: the sidebar's spinning record mark on its own (it
     is a solid accent disc, so it needs no chip), seated by a soft shadow so
     it holds on bright covers too. */
  .playing-mark {
    position: absolute;
    top: 8px;
    left: 8px;
    display: block;
    filter: drop-shadow(0 1px 2px rgb(0 0 0 / 0.45)) drop-shadow(0 3px 8px rgb(0 0 0 / 0.3));
    pointer-events: none;
  }
  .card.source .title {
    color: var(--accent);
  }

  /* Shuffle (glass) and Play (accent, the primary-transport glow, AR-1) rise
     from the cover's bottom-right corner on hover or keyboard focus. */
  .cover-actions {
    position: absolute;
    right: 10px;
    bottom: 10px;
    display: flex;
    align-items: center;
    gap: 8px;
    opacity: 0;
    transform: translateY(8px);
    transition:
      opacity var(--dur-state) var(--ease-enter),
      transform var(--dur-state) var(--ease-enter);
  }
  .card:hover .cover-actions,
  .card.split:focus-within .cover-actions {
    opacity: 1;
    transform: none;
  }
  .act {
    display: grid;
    place-items: center;
    border-radius: 50%;
    cursor: pointer;
    transition:
      transform var(--dur-micro) var(--ease-enter),
      filter var(--dur-micro) var(--ease-enter);
  }
  .act:hover {
    transform: scale(1.06);
    filter: brightness(1.08);
  }
  .act:focus-visible {
    outline: 2px solid var(--text);
    outline-offset: 2px;
  }
  .act.shuffle {
    width: 36px;
    height: 36px;
    color: var(--text);
    background: var(--surface-float);
    box-shadow: 0 0 0 1px var(--line);
    backdrop-filter: blur(var(--glass-blur));
  }
  .act.play {
    width: 44px;
    height: 44px;
    color: var(--on-accent);
    background: var(--accent);
    box-shadow: 0 0 10px color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .title {
    padding: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
    font-size: 16px;
    font-weight: 600;
    text-align: left;
  }
  button.name {
    cursor: pointer;
  }
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: color-mix(in srgb, var(--text) 62%, transparent);
    font-size: 14px;
  }
</style>
