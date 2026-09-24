<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Playlist as PlaylistT } from "../lib/api";
  import ConfirmDialog from "../lib/components/ConfirmDialog.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Notice } from "../lib/notice.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  let {
    nav,
    id,
    player,
    favorites,
    playlistActions,
    notice,
  }: {
    nav: Nav;
    id: string;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    notice: Notice;
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
      notice.show("Playlist renamed");
    } catch (e) {
      notice.show(String(e));
    }
  }

  async function confirmDelete() {
    confirmingDelete = false;
    try {
      await api.deletePlaylist(id);
      notice.show("Playlist deleted");
      nav.goto({ name: "library" });
    } catch (e) {
      notice.show(String(e));
    }
  }

  async function removeAt(index: number) {
    if (busy) return;
    busy = true;
    try {
      await api.removePlaylistItems(id, [index]);
      await pager.refresh();
      notice.show("Removed from playlist");
    } catch (e) {
      notice.show(String(e));
    } finally {
      busy = false;
    }
  }

  async function moveAt(index: number, delta: -1 | 1) {
    if (busy) return;
    busy = true;
    try {
      await api.movePlaylistItems(id, [index], index + delta);
      await pager.refresh();
    } catch (e) {
      notice.show(String(e));
    } finally {
      busy = false;
    }
  }

  async function playFrom(index: number) {
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(pager.items.map(trackToQueueItem), index);
  }

  async function playAll() {
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(pager.items.map(trackToQueueItem), 0);
  }
</script>

{#if loading}
  <p class="muted">Loading…</p>
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
  </p>
  <TrackTable
    tracks={pager.items}
    {player}
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
  {#if pager.loading}<p class="muted">Loading…</p>{/if}
  {#if confirmingDelete}
    <ConfirmDialog
      title="Delete playlist"
      message={`Delete "${playlist?.title ?? "this playlist"}"? This cannot be undone.`}
      onconfirm={confirmDelete}
      oncancel={() => (confirmingDelete = false)}
    />
  {/if}
{/if}
