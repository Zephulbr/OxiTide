<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Album, type Playlist } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  let { nav, player }: { nav: Nav; player: Player } = $props();

  let albums = $state<Album[]>([]);
  let playlists = $state<Playlist[]>([]);
  let loading = $state(true);
  let error = $state("");
  let sentinel = $state<HTMLElement | null>(null);

  const favorites = createTrackPager("favorites", (offset, limit) =>
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

  // Eager first page: the sentinel alone would leave the section empty when it
  // starts below the fold. `untrack` keeps this a one-shot mount action.
  $effect(() => {
    untrack(() => favorites.loadMore());
  });

  $effect(() => {
    if (!sentinel || favorites.done) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) favorites.loadMore();
    });
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

  async function playFrom(index: number) {
    await favorites.ensureAll();
    if (favorites.error) return;
    api.playTracks(favorites.items.map(trackToQueueItem), index);
  }

  async function playAll() {
    await favorites.ensureAll();
    if (favorites.error) return;
    api.playTracks(favorites.items.map(trackToQueueItem), 0);
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
    <button onclick={() => favorites.refresh()}>Refresh</button>
    <span class="muted">{favorites.items.length} of {favorites.total}</span>
  </p>
  <TrackTable tracks={favorites.items} {player} onPlay={playFrom} onPlayAll={playAll} />
  {#if favorites.error}
    <ErrorState message={favorites.error} onRetry={() => favorites.loadMore()} />
  {/if}
  {#if favorites.loading}<p class="muted">Loading…</p>{/if}
  {#if !favorites.done && !favorites.error}
    <div bind:this={sentinel} style="height:1px"></div>
  {/if}
{/if}
