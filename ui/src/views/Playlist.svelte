<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Playlist as PlaylistT } from "../lib/api";
  import ConfirmDialog from "../lib/components/ConfirmDialog.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PlaylistCover from "../lib/components/PlaylistCover.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import { invalidatePlaylistCovers } from "../lib/playlistArt.svelte";
  import { createTrackView, visibleIndexOf, warmTrackSearch } from "../lib/trackView.svelte";
  import type { CollectionTree } from "../lib/collection.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import { errorMessage } from "../lib/errors";
  import type { Toasts } from "../lib/toasts.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  let {
    nav,
    id,
    player,
    favorites,
    playlistActions,
    toasts,
    collection,
  }: {
    nav: Nav;
    id: string;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    toasts: Toasts;
    collection: CollectionTree;
  } = $props();

  let playlist = $state<PlaylistT | null>(null);
  let loading = $state(true);
  let error = $state("");
  let renaming = $state(false);
  let renameTitle = $state("");
  let renameDescription = $state("");
  let confirmingDelete = $state(false);
  let busy = $state(false);

  // `$derived` so navigating to another playlist rebuilds the pager; the new
  // key hydrates from the session cache when that playlist was seen before.
  const pager = $derived(
    createTrackPager(
      `playlist:${id}`,
      (offset, limit) => api.getPlaylistTracksPage(id, offset, limit),
      { dedupe: false, prefetchWindow: 100 },
    ),
  );

  // Rebuild the view when the pager is swapped for another playlist; reading
  // `pager` here makes that reactive (the closures alone would not).
  const view = $derived.by(() => {
    const p = pager;
    return createTrackView(() => p.items, async () => {
      await p.ensureAll();
      if (p.error) throw p.error;
    });
  });

  async function load() {
    loading = true;
    error = "";
    try {
      playlist = await api.getPlaylist(id);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });

  $effect(() => {
    const current = pager;
    untrack(() => current.loadMore());
  });

  // Warm the filter/sort caches in idle chunks for large playlists (see
  // CollectionTracks); the first keystroke must not pay the whole build.
  $effect(() => {
    const list = pager.items;
    if (list.length < 500) return;
    return warmTrackSearch(list).cancel;
  });

  function startRename() {
    renaming = true;
    renameTitle = playlist?.title ?? "";
    renameDescription = playlist?.description ?? "";
  }

  async function saveRename() {
    const title = renameTitle.trim();
    if (!title) return;
    try {
      await api.renamePlaylist(id, title, renameDescription.trim());
      if (playlist) {
        playlist.title = title;
        playlist.description = renameDescription.trim() || null;
      }
      renaming = false;
      toasts.show("Playlist renamed", { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
  }

  async function confirmDelete() {
    confirmingDelete = false;
    try {
      await api.deletePlaylist(id);
      invalidatePlaylistCovers(id);
      await collection.refresh();
      toasts.show("Playlist deleted", { severity: "success" });
      nav.goto({ name: "playlists" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
  }

  async function removeAt(index: number) {
    if (busy) return;
    const sourceIndex = view.sourceIndex(index);
    if (sourceIndex < 0) return;
    busy = true;
    try {
      await api.removePlaylistItems(id, [sourceIndex]);
      invalidatePlaylistCovers(id);
      await pager.refresh();
      toasts.show("Removed from playlist", { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  async function moveTrack(from: number, to: number) {
    if (busy || from === to) return;
    busy = true;
    pager.moveItem(from, to);
    try {
      await api.movePlaylistItems(id, [from], to);
      invalidatePlaylistCovers(id);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
      await pager.refresh();
    } finally {
      busy = false;
    }
  }

  function moveAt(index: number, delta: -1 | 1) {
    return moveTrack(index, index + delta);
  }

  async function playFrom(index: number) {
    const clicked = view.items[index];
    await pager.ensureAll();
    if (pager.error) return;
    const start = visibleIndexOf(view.items, clicked, index);
    if (start < 0) return;
    playSource.set(id);
    api.playTracks(view.items.map(trackToQueueItem), start);
  }

  async function playAll() {
    await pager.ensureAll();
    if (pager.error) return;
    playSource.set(id);
    api.playTracks(view.items.map(trackToQueueItem), 0);
  }

  async function shuffleAll() {
    await pager.ensureAll();
    if (pager.error) return;
    playSource.set(id);
    api.playTracks(view.items.map(trackToQueueItem), 0, true);
  }
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else}
  <div class="view-fill">
    {#if playlist}
      <header class="view-header">
        <span class="header-art">
          <PlaylistCover
            owned={playlist.owned}
            playlist={id}
            image={playlist.squareImage}
            tracks={pager.items}
            size={320}
            radius={12}
            alt={`${playlist.title} cover`}
          />
        </span>
        <div class="header-meta">
          <h2>{playlist.title}</h2>
          {#if playlist.owned}
            <div class="header-actions">
              <button onclick={startRename}>Rename</button>
              <button onclick={() => (confirmingDelete = true)}>Delete</button>
            </div>
          {/if}
        </div>
      </header>
      <div class="header-buttons">
        <button class="btn-play" onclick={playAll} disabled={view.items.length === 0}>
          <Icon name="play" size={18} /> Play
        </button>
        <button class="btn-shuffle" onclick={shuffleAll} disabled={view.items.length === 0}>
          <Icon name="shuffle" size={18} /> Shuffle
        </button>
      </div>
    {/if}
    {#if renaming}
      <p>
        <input bind:value={renameTitle} aria-label="Playlist name" />
        <input bind:value={renameDescription} aria-label="Playlist description" />
        <button onclick={saveRename}>Save</button>
        <button onclick={() => (renaming = false)}>Cancel</button>
      </p>
    {/if}
    <p>
      <button
        onclick={() => {
          invalidatePlaylistCovers(id);
          void pager.refresh();
        }}
      >Refresh</button>
      <span class="muted">
        {#if pager.unavailable > 0}
          {pager.items.length} tracks ({pager.unavailable} unavailable)
        {:else}
          {pager.items.length} tracks
        {/if}
      </span>
      {#if pager.loading}<Loading />{/if}
    </p>
    <TrackTable
      {view}
      {favorites}
      {toasts}
      fill
      onPlay={playFrom}
      onEndReached={() => void pager.loadMore()}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
      playingTrackId={player.state.current?.track_id ?? null}
      playlistActions={playlist?.owned
        ? { onRemove: removeAt, onMove: moveAt, busy, disabled: !pager.done, onDrop: moveTrack }
        : undefined}
    />
    {#if pager.error}
      <ErrorState message={pager.error} onRetry={() => pager.loadMore()} />
    {/if}
  </div>
  {#if confirmingDelete}
    <ConfirmDialog
      title="Delete playlist"
      message={`Delete "${playlist?.title ?? "this playlist"}"? This cannot be undone.`}
      onconfirm={confirmDelete}
      oncancel={() => (confirmingDelete = false)}
    />
  {/if}
{/if}

<style>
  .view-header {
    display: flex;
    align-items: flex-end;
    gap: 20px;
    margin-bottom: 14px;
  }
  .header-art {
    flex: 0 0 auto;
    width: 160px;
  }
  /* Big labeled actions under the artwork (screenshot layout, app tokens). */
  .header-buttons {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 18px;
  }
  .header-buttons button {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 22px;
    border-radius: var(--radius-control);
    font-weight: 600;
  }
  .btn-play {
    background: var(--accent);
    color: var(--on-accent);
  }
  .btn-play :global(svg) {
    fill: currentColor;
  }
  .btn-shuffle {
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .btn-shuffle:hover {
    color: var(--text);
  }
  .header-meta {
    min-width: 0;
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }
</style>
