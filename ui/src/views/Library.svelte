<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Album, type Playlist } from "../lib/api";
  import ConfirmDialog from "../lib/components/ConfirmDialog.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import VirtualList from "../lib/components/VirtualList.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import { createTrackView, visibleIndexOf } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import { errorMessage } from "../lib/errors";
  import { menuFor } from "../lib/menu.svelte";
  import { libraryPlaylistMenuItems } from "../lib/menus";
  import type { Toasts } from "../lib/toasts.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  let {
    nav,
    player,
    favorites,
    playlistActions,
    toasts,
  }: {
    nav: Nav;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    toasts: Toasts;
  } = $props();

  let albums = $state<Album[]>([]);
  let playlists = $state<Playlist[]>([]);
  let loading = $state(true);
  let error = $state("");
  let editingUuid = $state<string | null>(null);
  let editTitle = $state("");
  let deleting = $state<Playlist | null>(null);
  // Captured at mount so the revision effect only refreshes after a create
  // that happens while this view is alive.
  let revisionSeen = untrack(() => playlistActions.revision);

  const favoritesPager = createTrackPager("favorites", (offset, limit) =>
    api.getFavoriteTracksPage(offset, limit),
  );

  async function ensureAllFavorites() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) throw favoritesPager.error;
  }

  const view = createTrackView(() => favoritesPager.items, ensureAllFavorites);

  async function load() {
    loading = true;
    error = "";
    try {
      [albums, playlists] = await Promise.all([api.getFavoriteAlbums(), api.getUserPlaylists()]);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function loadPlaylists() {
    try {
      playlists = await api.getUserPlaylists();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
  }

  $effect(() => {
    load();
  });

  // The first page paints fast, then the pager prefetches the remainder in the
  // background. `untrack` keeps this a one-shot mount action.
  $effect(() => {
    untrack(() => favoritesPager.loadMore());
  });

  // An unfavorite from anywhere (this table, the player bar, another view)
  // removes the row from the loaded list without refetching — unless it has
  // since been re-favorited, in which case it stays.
  $effect(() => {
    favorites.trackRevision;
    const loaded = new Set(favoritesPager.items.map((t) => t.id));
    for (const id of favorites.pendingRemovalsToDrop(loaded)) favoritesPager.removeItem(id);
  });

  // A playlist created from the picker while this view is mounted.
  $effect(() => {
    const revision = playlistActions.revision;
    if (revision === revisionSeen) return;
    revisionSeen = revision;
    untrack(() => void loadPlaylists());
  });

  function startRename(p: Playlist) {
    editingUuid = p.uuid;
    editTitle = p.title;
  }

  async function saveRename(p: Playlist) {
    const title = editTitle.trim();
    if (!title) return;
    try {
      await api.renamePlaylist(p.uuid, title, p.description ?? "");
      playlists = playlists.map((item) => (item.uuid === p.uuid ? { ...item, title } : item));
      editingUuid = null;
      toasts.show("Playlist renamed", { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
  }

  async function confirmDelete() {
    const target = deleting;
    if (!target) return;
    try {
      await api.deletePlaylist(target.uuid);
      playlists = playlists.filter((item) => item.uuid !== target.uuid);
      toasts.show(`Deleted ${target.title}`, { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      deleting = null;
    }
  }

  async function playFrom(index: number) {
    const clicked = view.items[index];
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    const start = visibleIndexOf(view.items, clicked, index);
    if (start < 0) return;
    api.playTracks(view.items.map(trackToQueueItem), start);
  }

  async function playAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    api.playTracks(view.items.map(trackToQueueItem), 0);
  }
</script>

<h2>Library</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else}
  <h3>
    Playlists
    <button onclick={() => playlistActions.openCreate()}>New playlist</button>
  </h3>
  <div role="list">
    <VirtualList total={playlists.length} rowHeight={44} maxHeight="min(40vh, 360px)">
      {#snippet children(index)}
        {@const p = playlists[index]}
        {#if p}
          <div
            role="listitem"
            style="height:44px;box-sizing:border-box;display:flex;align-items:center;gap:8px"
            use:menuFor={() =>
              libraryPlaylistMenuItems({
                playlist: p,
                editable: p.owned,
                open: () => nav.goto({ name: "playlist", id: p.uuid }),
                rename: () => startRename(p),
                remove: () => (deleting = p),
              })}
          >
            <span class="lib-thumb"><Cover uuid={p.image} size={80} radius={6} alt="" /></span>
            {#if editingUuid === p.uuid}
              <input bind:value={editTitle} aria-label="Playlist name" />
              <button onclick={() => void saveRename(p)}>Save</button>
              <button onclick={() => (editingUuid = null)}>Cancel</button>
            {:else}
              <button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button>
              {#if p.owned}
                <button onclick={() => startRename(p)}>Rename</button>
                <button onclick={() => (deleting = p)}>Delete</button>
              {/if}
            {/if}
          </div>
        {/if}
      {/snippet}
    </VirtualList>
  </div>
  {#if playlists.length === 0}<p class="muted">No playlists</p>{/if}
  <h3>Albums</h3>
  <div role="list">
    <VirtualList total={albums.length} rowHeight={36} maxHeight="min(40vh, 360px)">
      {#snippet children(index)}
        {@const a = albums[index]}
        {#if a}
          <div
            role="listitem"
            style="height:36px;box-sizing:border-box;display:flex;align-items:center;gap:8px"
          >
            <span class="lib-thumb"><Cover uuid={a.cover} size={80} radius={6} alt="" /></span>
            <button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button>
          </div>
        {/if}
      {/snippet}
    </VirtualList>
  </div>
  {#if albums.length === 0}<p class="muted">No albums</p>{/if}
  <h3>Favorite tracks</h3>
  <p>
    <button onclick={() => favoritesPager.refresh()}>Refresh</button>
    <span class="muted">
      {#if favoritesPager.unavailable > 0}
        {favoritesPager.items.length} tracks ({favoritesPager.unavailable} unavailable)
      {:else}
        {favoritesPager.items.length} tracks
      {/if}
    </span>
    {#if favoritesPager.loading}<Loading />{/if}
  </p>
  <TrackTable
    {view}
    {favorites}
    onPlay={playFrom}
    onPlayAll={playAll}
    onEndReached={() => void favoritesPager.loadMore()}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    playingTrackId={player.state.current?.track_id ?? null}
  />
  {#if favoritesPager.error}
    <ErrorState message={favoritesPager.error} onRetry={() => favoritesPager.loadMore()} />
  {/if}
  {#if deleting}
    <ConfirmDialog
      title="Delete playlist"
      message={`Delete "${deleting.title}"? This cannot be undone.`}
      onconfirm={confirmDelete}
      oncancel={() => (deleting = null)}
    />
  {/if}
{/if}

<style>
  .lib-thumb {
    flex: 0 0 auto;
    width: var(--thumb-size);
  }
</style>
