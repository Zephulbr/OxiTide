<script lang="ts">
  import { api } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createSearch } from "../lib/search.svelte";
  import { createTrackView } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  let {
    nav,
    player,
    favorites,
    playlistActions,
  }: { nav: Nav; player: Player; favorites: Favorites; playlistActions: PlaylistActions } = $props();

  const search = createSearch(api.search);
  const resultsView = createTrackView(() => search.results?.tracks.items ?? []);

  const noResults = $derived(
    search.results !== null &&
      search.results.tracks.items.length === 0 &&
      search.results.albums.items.length === 0 &&
      search.results.artists.items.length === 0 &&
      search.results.playlists.items.length === 0,
  );
</script>

<h2>Search</h2>
<div class="search-bar">
  <input
    id="search-input"
    value={search.query}
    oninput={(event) => search.setQuery(event.currentTarget.value)}
    onkeydown={(event) => event.key === "Enter" && search.runNow()}
    placeholder="Search TIDAL"
  />
  {#if search.loading && search.results}
    <Loading label="Searching…" />
  {/if}
</div>
{#if search.loading && !search.results && !search.error}
  <Loading label="Searching…" />
{:else if search.error}
  <ErrorState message={search.error} onRetry={() => search.retry()} />
{:else if noResults}
  <EmptyState message={`No results for “${search.settledQuery}”`} />
{:else if search.results}
  <h3>Tracks</h3>
  <TrackTable
    view={resultsView}
    {favorites}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
  />
  <h3>Albums</h3>
  <ul>
    {#each search.results.albums.items as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button></li>
    {/each}
  </ul>
  <h3>Artists</h3>
  <ul>
    {#each search.results.artists.items as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "artist", id: a.id })}>{a.name}</button></li>
    {/each}
  </ul>
  <h3>Playlists</h3>
  <ul>
    {#each search.results.playlists.items as p (p.uuid)}
      <li><button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button></li>
    {/each}
  </ul>
{/if}

<style>
  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
