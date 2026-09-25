<script lang="ts">
  import { api, type SearchResults } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
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
  let query = $state("");
  let results = $state<SearchResults | null>(null);
  let loading = $state(false);
  let error = $state("");
  let searchedQuery = $state("");

  const resultsView = createTrackView(() => results?.tracks.items ?? []);

  const noResults = $derived(
    results !== null &&
      results.tracks.items.length === 0 &&
      results.albums.items.length === 0 &&
      results.artists.items.length === 0 &&
      results.playlists.items.length === 0,
  );

  async function load() {
    if (query.trim().length === 0) return;
    loading = true;
    error = "";
    try {
      results = await api.search(query);
      searchedQuery = query;
    } catch (e) {
      error = String(e);
      results = null;
    } finally {
      loading = false;
    }
  }
</script>

<h2>Search</h2>
<input
  id="search-input"
  bind:value={query}
  onkeydown={(e) => e.key === "Enter" && load()}
  placeholder="Search TIDAL"
/>
{#if loading}
  <Loading label="Searching…" />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if noResults}
  <EmptyState message={`No results for “${searchedQuery}”`} />
{:else if results}
  <h3>Tracks</h3>
    <TrackTable
      view={resultsView}
      {favorites}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    />
  <h3>Albums</h3>
  <ul>
    {#each results.albums.items as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button></li>
    {/each}
  </ul>
  <h3>Artists</h3>
  <ul>
    {#each results.artists.items as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "artist", id: a.id })}>{a.name}</button></li>
    {/each}
  </ul>
  <h3>Playlists</h3>
  <ul>
    {#each results.playlists.items as p (p.uuid)}
      <li><button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button></li>
    {/each}
  </ul>
{/if}
