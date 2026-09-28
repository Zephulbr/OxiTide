<script lang="ts">
  import { api, trackToQueueItem, type PageSection } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import PlaylistCover from "../lib/components/PlaylistCover.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { playMix } from "../lib/mixPlay";
  import { createSearch } from "../lib/search.svelte";
  import { createTrackView } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

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

  const search = createSearch(api.search);
  const resultsView = createTrackView(() => search.results?.tracks.items ?? []);

  let sections = $state<PageSection[]>([]);
  let exploreLoading = $state(true);
  let exploreError = $state("");

  async function loadExplore() {
    exploreLoading = true;
    exploreError = "";
    try {
      sections = await api.getExplore();
    } catch (e) {
      exploreError = String(e);
    } finally {
      exploreLoading = false;
    }
  }

  $effect(() => {
    loadExplore();
  });

  const searching = $derived(search.query.trim().length > 0);

  const noResults = $derived(
    search.results !== null &&
      search.results.tracks.items.length === 0 &&
      search.results.albums.items.length === 0 &&
      search.results.artists.items.length === 0 &&
      search.results.playlists.items.length === 0,
  );

  function playAll() {
    if (resultsView.items.length === 0) return;
    void api.playTracks(resultsView.items.map(trackToQueueItem), 0);
  }

  function shuffleAll() {
    if (resultsView.items.length === 0) return;
    void api.playTracks(resultsView.items.map(trackToQueueItem), 0, true);
  }
</script>

<h2>Explore</h2>
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

{#if searching}
  {#if search.loading && !search.results && !search.error}
    <Loading label="Searching…" />
  {:else if search.error}
    <ErrorState message={search.error} onRetry={() => search.retry()} />
  {:else if noResults}
    <EmptyState message={`No results for “${search.settledQuery}”`} />
  {:else if search.results}
    <div class="list-head">
      <h3>Tracks</h3>
      <button
        class="list-action"
        onclick={shuffleAll}
        disabled={resultsView.items.length === 0}
        aria-label="Shuffle"
        title="Shuffle"
      ><Icon name="shuffle" size={16} /></button>
      <button
        class="list-action"
        onclick={playAll}
        disabled={resultsView.items.length === 0}
        aria-label="Play all"
        title="Play all"
      ><Icon name="play" size={16} /></button>
    </div>
    <TrackTable
      view={resultsView}
      {favorites}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
      playingTrackId={player.state.current?.track_id ?? null}
    />
    <h3>Albums</h3>
    <ul class="results">
      {#each search.results.albums.items as a (a.id)}
        <li>
          <span class="res-thumb"><Cover uuid={a.cover} size={80} radius={6} alt="" /></span>
          <button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button>
        </li>
      {/each}
    </ul>
    <h3>Artists</h3>
    <ul class="results">
      {#each search.results.artists.items as a (a.id)}
        <li>
          <span class="res-thumb round"><Cover uuid={a.picture} size={80} radius={50} alt="" /></span>
          <button onclick={() => nav.goto({ name: "artist", id: a.id })}>{a.name}</button>
        </li>
      {/each}
    </ul>
    <h3>Playlists</h3>
    <ul class="results">
      {#each search.results.playlists.items as p (p.uuid)}
        <li>
          <span class="res-thumb">
            <PlaylistCover
              owned={p.owned}
              playlist={p.uuid}
              image={p.squareImage}
              size={80}
              radius={6}
              alt=""
            />
          </span>
          <button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button>
        </li>
      {/each}
    </ul>
  {/if}
{:else if exploreLoading}
  <Loading />
{:else if exploreError}
  <ErrorState message={exploreError} onRetry={loadExplore} />
{:else if sections.length === 0}
  <EmptyState message="Nothing to explore yet." />
{:else}
  <PageSections {sections} {nav} onPlayMix={(id) => void playMix(id, toasts)} />
{/if}

<style>
  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .list-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .list-head h3 {
    flex: 1;
    min-width: 0;
  }
  .list-action {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .list-action:hover {
    color: var(--text);
  }
  .results {
    list-style: none;
    margin: 0 0 16px;
    padding: 0;
  }
  .results li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 0;
  }
  .res-thumb {
    flex: 0 0 auto;
    width: var(--thumb-size);
  }
</style>
