<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Playlist as PlaylistT } from "../lib/api";
  import ConfirmDialog from "../lib/components/ConfirmDialog.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import { createTrackView } from "../lib/trackView.svelte";
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
  }: {
    nav: Nav;
    id: string;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    toasts: Toasts;
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

  async function ensureAllTracks() {
    await pager.ensureAll();
    if (pager.error) throw new Error(pager.error);
  }

  const view = $derived(createTrackView(() => pager.items, ensureAllTracks));

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
      toasts.show("Playlist deleted", { severity: "success" });
      nav.goto({ name: "library" });
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
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(view.items.map(trackToQueueItem), index);
  }

  async function playAll() {
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(view.items.map(trackToQueueItem), 0);
  }
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else}
  {#if playlist}
    <h2>
      {playlist.title}
      <button onclick={startRename}>Rename</button>
      <button onclick={() => (confirmingDelete = true)}>Delete</button>
    </h2>
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
    <button onclick={() => pager.refresh()}>Refresh</button>
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
    onPlay={playFrom}
    onPlayAll={playAll}
    onEndReached={() => void pager.loadMore()}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    playlistActions={{ onRemove: removeAt, onMove: moveAt, busy, disabled: !pager.done }}
  />
  {#if pager.error}
    <ErrorState message={pager.error} onRetry={() => pager.loadMore()} />
  {/if}
  {#if confirmingDelete}
    <ConfirmDialog
      title="Delete playlist"
      message={`Delete "${playlist?.title ?? "this playlist"}"? This cannot be undone.`}
      onconfirm={confirmDelete}
      oncancel={() => (confirmingDelete = false)}
    />
  {/if}
{/if}
