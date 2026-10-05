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
  import { playSource } from "../lib/playSource.svelte";
  import { trackFlags } from "../lib/trackFlags.svelte";
  import { createSearch } from "../lib/search.svelte";
  import { searchHistory } from "../lib/searchHistory.svelte";
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
  const history = searchHistory();

  let searchFocused = $state(false);
  let suggestionsDismissed = $state(false);
  let activeSuggestion = $state(-1);

  const suggestions = $derived(history.matches(search.query));
  const showSuggestions = $derived(
    searchFocused && !suggestionsDismissed && suggestions.length > 0,
  );
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

  function onSearchInput(value: string) {
    suggestionsDismissed = false;
    activeSuggestion = -1;
    search.setQuery(value);
  }

  function chooseSuggestion(value: string) {
    history.record(value);
    search.setQuery(value);
    search.runNow();
    suggestionsDismissed = true;
    activeSuggestion = -1;
  }

  function onSearchKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown" && suggestions.length > 0) {
      event.preventDefault();
      suggestionsDismissed = false;
      activeSuggestion = activeSuggestion < suggestions.length - 1 ? activeSuggestion + 1 : 0;
    } else if (event.key === "ArrowUp" && suggestions.length > 0) {
      event.preventDefault();
      suggestionsDismissed = false;
      activeSuggestion = activeSuggestion > 0 ? activeSuggestion - 1 : suggestions.length - 1;
    } else if (event.key === "Enter") {
      if (showSuggestions && activeSuggestion >= 0) {
        event.preventDefault();
        chooseSuggestion(suggestions[activeSuggestion]);
      } else {
        history.record(search.query);
        search.runNow();
      }
    } else if (event.key === "Escape" && showSuggestions) {
      event.preventDefault();
      suggestionsDismissed = true;
      activeSuggestion = -1;
    }
  }

  function clearHistory() {
    history.clear();
    activeSuggestion = -1;
  }

  async function playFrom(index: number) {
    const prepared = await trackFlags.filterPlayable(resultsView.items, index);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function playAll() {
    if (resultsView.items.length === 0) return;
    const prepared = await trackFlags.filterPlayable(resultsView.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function shuffleAll() {
    if (resultsView.items.length === 0) return;
    const prepared = await trackFlags.filterPlayable(resultsView.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), 0, true);
  }
</script>

<h2>Explore</h2>
<div class="search-bar">
  <span class="search-icon" aria-hidden="true"><Icon name="search" size={16} /></span>
  <input
    id="search-input"
    name="oxitide-search"
    class="search-input search-bar-input"
    value={search.query}
    oninput={(event) => onSearchInput(event.currentTarget.value)}
    onkeydown={onSearchKeydown}
    onfocus={() => {
      searchFocused = true;
      suggestionsDismissed = false;
    }}
    onblur={() => {
      searchFocused = false;
      activeSuggestion = -1;
    }}
    placeholder="Search TIDAL"
    autocomplete="off"
    spellcheck="false"
    aria-expanded={showSuggestions}
    aria-controls="search-suggestions"
  />
  {#if search.loading && search.results}
    <Loading label="Searching…" />
  {/if}
  {#if showSuggestions}
    <div
      class="suggest"
      id="search-suggestions"
      role="listbox"
      aria-label="Recent searches"
    >
      <div class="suggest-head">
        <span>Recent searches</span>
        <button
          type="button"
          class="suggest-clear"
          aria-label="Clear recent searches"
          title="Clear recent searches"
          onmousedown={(event) => event.preventDefault()}
          onclick={clearHistory}
        ><Icon name="x" size={14} /></button>
      </div>
      {#each suggestions as suggestion, i (suggestion)}
        <button
          type="button"
          role="option"
          aria-selected={i === activeSuggestion}
          class:active={i === activeSuggestion}
          onmouseenter={() => (activeSuggestion = i)}
          onmouseleave={() => (activeSuggestion = -1)}
          onmousedown={(event) => event.preventDefault()}
          onclick={() => chooseSuggestion(suggestion)}
        >
          <span class="suggest-icon"><Icon name="search" size={14} /></span>
          <span class="suggest-text">{suggestion}</span>
        </button>
      {/each}
    </div>
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
      {toasts}
      onPlay={playFrom}
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
  <PageSections {sections} {nav} {toasts} source="explore" />
{/if}

<style>
  .search-bar {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 560px;
  }
  .search-bar-input {
    flex: 1;
    min-width: 0;
    padding-left: 36px;
  }
  .search-icon {
    position: absolute;
    left: 12px;
    display: grid;
    place-items: center;
    color: var(--text-muted);
    pointer-events: none;
  }
  /* Recent-search dropdown: the floating-overlay recipe (darker float veil +
     slight accent tint) shared with ContextMenu, so both stay legible over
     bright art and read as one overlay family. */
  .suggest {
    --suggest-fill: color-mix(in srgb, var(--accent) 8%, var(--surface-float));
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 10;
    min-width: 260px;
    max-width: 420px;
    padding: 4px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background:
      linear-gradient(var(--suggest-fill), var(--suggest-fill)) padding-box,
      var(--edge-lit) border-box;
    backdrop-filter: blur(var(--glass-blur));
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.4);
  }
  .suggest-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 4px 4px 10px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .suggest-clear {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    color: var(--text-muted);
  }
  .suggest-clear:hover {
    color: var(--text);
  }
  .suggest > button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border-radius: 4px;
    text-align: left;
  }
  .suggest-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    color: var(--text-muted);
  }
  .suggest > button:hover,
  .suggest > button.active {
    background: var(--surface-2);
  }
  .suggest-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
