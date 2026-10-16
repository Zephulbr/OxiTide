<script lang="ts">
  import { api, trackToQueueItem, type HistoryEntry, type PageItem, type PageSection } from "../lib/api";
  import Carousel from "../lib/components/Carousel.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import { buildNewToYou, topArtistNames, type NewArtist } from "../lib/exploreSuggest";
  import { routeForPageItem } from "../lib/nav.svelte";
  import { splitQuickLinks, withoutVideoLinks } from "../lib/pageLinks";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import PlaylistCover from "../lib/components/PlaylistCover.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { isAlbumRelease } from "../lib/media";
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

  /// Personal parts (chips, New to you) load on their own and stay hidden on
  /// failure or an empty history, like Home's shelves.
  let listened = $state<HistoryEntry[]>([]);
  let newToYou = $state<NewArtist[]>([]);

  async function loadPersonal() {
    try {
      listened = (await api.getHistoryPage(0, 200)).items;
      newToYou = await buildNewToYou(listened, api, (id) => favorites.hasArtist(id));
    } catch {
      newToYou = [];
    } finally {
      personalSettled = true;
    }
  }

  /// The body (chips, New to you, TIDAL's rows) reveals in one go once the
  /// personal parts and the explore rows have settled, or after REVEAL_CAP_MS.
  /// They answer in any order, so shown on arrival New to you could land
  /// above rows already on screen and shove them down. The title and search
  /// field never wait.
  const REVEAL_CAP_MS = 2500;
  let personalSettled = $state(false);
  let capped = $state(false);
  const ready = $derived((!exploreLoading && personalSettled) || capped);

  $effect(() => {
    loadExplore();
    void loadPersonal();
    const timer = setTimeout(() => (capped = true), REVEAL_CAP_MS);
    return () => clearTimeout(timer);
  });

  /// Enough to fill the row under the search field; the row clips whatever
  /// does not fit (CSS), so the count is a ceiling, not a layout input.
  const CHIP_CAP = 16;

  /// Recent searches first, then top artists from history not already there.
  const chips = $derived.by(() => {
    const out: { value: string; recent: boolean }[] = [];
    const taken = new Set<string>();
    const add = (value: string, recent: boolean) => {
      const key = value.trim().toLowerCase();
      if (!key || taken.has(key) || out.length >= CHIP_CAP) return;
      taken.add(key);
      out.push({ value, recent });
    };
    for (const entry of history.entries.slice(0, 5)) add(entry, true);
    for (const name of topArtistNames(listened, CHIP_CAP)) add(name, false);
    return out;
  });

  const newItems = $derived<PageItem[]>(
    newToYou.map(({ artist, seed }) => ({
      id: String(artist.id),
      title: artist.name,
      subtitle: `Like ${seed}`,
      item_type: "ARTIST",
      artwork: artist.picture ?? null,
    })),
  );

  /// Untitled link rows (New, Top, HiRes, …) sit under "New to you"; video
  /// links are dropped everywhere (OxiTide does not play videos).
  const arranged = $derived(splitQuickLinks(withoutVideoLinks(sections)));

  function open(item: PageItem): (() => void) | undefined {
    const route = routeForPageItem(item);
    return route ? () => nav.goto(route) : undefined;
  }

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

<header class="page-head explore-head">
<h2>Explore</h2>
<div class="search-block">
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
    aria-label="Search TIDAL"
    autocomplete="off"
    spellcheck="false"
    role="combobox"
    aria-autocomplete="list"
    aria-expanded={showSuggestions}
    aria-controls="search-suggestions"
    aria-activedescendant={showSuggestions && activeSuggestion >= 0
      ? `search-suggestion-${activeSuggestion}`
      : undefined}
  />
  {#if search.loading && search.results}
    <Loading label="Searching…" />
  {/if}
  {#if showSuggestions}
    <div class="suggest">
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
      <!-- The listbox wraps only the options; focus stays in the combobox. -->
      <div id="search-suggestions" role="listbox" aria-label="Recent searches">
        {#each suggestions as suggestion, i (suggestion)}
          <button
            id={`search-suggestion-${i}`}
            type="button"
            role="option"
            tabindex="-1"
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
    </div>
  {/if}
</div>
{#if ready && !searching && chips.length > 0}
  <div class="chips" role="group" aria-label="Search suggestions">
    {#each chips as chip (chip.value)}
      <button
        type="button"
        class="chip"
        onclick={() => chooseSuggestion(chip.value)}
        title={chip.recent ? `Search again for “${chip.value}”` : `Search for ${chip.value}`}
      >
        <Icon name={chip.recent ? "history" : "user"} size={13} />
        <span>{chip.value}</span>
      </button>
    {/each}
  </div>
{/if}
</div>
</header>

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
      {nav}
      {favorites}
      {toasts}
      onPlay={playFrom}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
      playingTrackId={player.state.current?.track_id ?? null}
      playbackRunning={player.state.playing}
    />
    <h3>Albums</h3>
    <ul class="results">
      {#each search.results.albums.items as a (a.id)}
        <li>
          <span class="res-thumb"><Cover uuid={a.cover} size={80} radius={isAlbumRelease(a.type) ? 0 : 6} alt="" /></span>
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
{:else if !ready}
  <div class="pending"><Loading /></div>
{:else}
  {#if newItems.length > 0}
    <section class="shelf reveal">
      <Carousel title="New to you">
        {#each newItems as item (item.id)}
          <MediaCard {item} variant="carousel" onclick={open(item)} />
        {/each}
      </Carousel>
    </section>
  {/if}
  {#if exploreLoading}
    <div class="pending"><Loading /></div>
  {:else if exploreError}
    <ErrorState message={exploreError} onRetry={loadExplore} />
  {:else if arranged.quick.length + arranged.rest.length === 0}
    <EmptyState message="Nothing to explore yet." />
  {:else}
    <div class="reveal later">
      <PageSections sections={[...arranged.quick, ...arranged.rest]} {nav} {player} {toasts} source="explore" />
    </div>
  {/if}
{/if}

<style>
  /* Explore's header is its search: the title keeps every page's h2 step,
     the field below it is the page's hero (spec 2026-10-16 Explore). */
  .explore-head {
    flex-direction: column;
    flex-wrap: nowrap;
    align-items: stretch;
    gap: 12px;
    margin-bottom: 26px;
  }
  /* The search is the page's centrepiece: centred, with its chips clipped to
     one row of the same width. */
  .search-block {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: min(680px, 100%);
    margin-inline: auto;
  }
  .search-bar {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .search-bar-input {
    flex: 1;
    min-width: 0;
    height: 48px;
    padding-left: 44px;
    font-size: 15px;
    box-shadow: var(--shadow-panel);
  }
  .search-icon {
    position: absolute;
    left: 16px;
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
  .suggest [role="option"] {
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
  .suggest [role="option"]:hover,
  .suggest [role="option"].active {
    background: var(--surface-2);
  }
  .suggest-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* One row: chips that would wrap are clipped (CSS, not a measured count,
     so the queue glide sees the final layout). Centred under the field. */
  .chips {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    /* 30px chips plus room for the focus ring; a wrapped row starts below. */
    height: 34px;
    padding: 2px;
    overflow: hidden;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 240px;
    height: 30px;
    padding: 0 12px 0 10px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--surface-1);
    color: var(--text-muted);
    font-size: 13px;
    transition:
      background var(--dur-state) var(--ease-enter),
      color var(--dur-state) var(--ease-enter);
  }
  .chip span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chip:hover,
  .chip:focus-visible {
    background: var(--surface-3);
    color: var(--text);
  }
  .shelf {
    margin-bottom: 10px;
  }
  /* The body's reveal (see `ready`): chips, New to you, then TIDAL's rows. */
  .reveal,
  .chips {
    animation: explore-in var(--dur-view) var(--ease-enter) backwards;
  }
  .reveal.later {
    animation-delay: 60ms;
  }
  @keyframes explore-in {
    from {
      opacity: 0;
      transform: translateY(var(--rise));
    }
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
