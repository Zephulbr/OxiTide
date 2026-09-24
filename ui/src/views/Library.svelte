<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Album, type Playlist } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  let { nav, player, favorites }: { nav: Nav; player: Player; favorites: Favorites } = $props();

  let albums = $state<Album[]>([]);
  let playlists = $state<Playlist[]>([]);
  let loading = $state(true);
  let error = $state("");

  const favoritesPager = createTrackPager("favorites", (offset, limit) =>
    api.getFavoriteTracksPage(offset, limit),
  );

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

  async function playFrom(index: number) {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    api.playTracks(favoritesPager.items.map(trackToQueueItem), index);
  }

  async function playAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    api.playTracks(favoritesPager.items.map(trackToQueueItem), 0);
  }
</script>

<h2>Library</h2>
{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else}
  <h3>Playlists</h3>
  <ul>
    {#each playlists as p (p.uuid)}
      <li><button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button></li>
    {/each}
    {#if playlists.length === 0}<li class="muted">No playlists</li>{/if}
  </ul>
  <h3>Albums</h3>
  <ul>
    {#each albums as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button></li>
    {/each}
    {#if albums.length === 0}<li class="muted">No albums</li>{/if}
  </ul>
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
  </p>
  <TrackTable
    tracks={favoritesPager.items}
    {player}
    {favorites}
    onPlay={playFrom}
    onPlayAll={playAll}
    onEndReached={() => void favoritesPager.loadMore()}
  />
  {#if favoritesPager.error}
    <ErrorState message={favoritesPager.error} onRetry={() => favoritesPager.loadMore()} />
  {/if}
  {#if favoritesPager.loading}<p class="muted">Loading…</p>{/if}
{/if}
