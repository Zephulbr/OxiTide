<script lang="ts">
  import { untrack } from "svelte";
  import { api, type HistoryEntry, type PageItem, type PageSection, type SearchKind } from "../lib/api";
  import Carousel from "../lib/components/Carousel.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import SearchResultsView from "../lib/components/SearchResultsView.svelte";
  import { buildNewToYou, topArtistNames, type NewArtist } from "../lib/exploreSuggest";
  import { routeForPageItem } from "../lib/nav.svelte";
  import { splitQuickLinks, withoutVideoLinks } from "../lib/pageLinks";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import SearchField from "../lib/components/SearchField.svelte";
  import { createSearch } from "../lib/search.svelte";
  import { searchHistory } from "../lib/searchHistory.svelte";
  import { createSearchPages } from "../lib/searchPages.svelte";
  import { kindsWithResults, searchCorrecting } from "../lib/searchResults";
  import { fetchShared, load as loadCached, peek } from "../lib/viewCache";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav, Route } from "../lib/nav.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";
  import { t, tSplit, type MessageKey } from "../lib/i18n.svelte";

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

  /// Searches go through the page cache, so Back to a search shows its
  /// results on the first frame instead of searching again. A typo that only
  /// finds its top hit searches again under the top hit's name
  /// (`searchCorrecting`); the entry keeps the query its results answer.
  const searchSource = (query: string) => ({
    key: `search:${query}`,
    fetch: () => searchCorrecting(api.search, query),
  });
  const search = createSearch(async (query) => (await fetchShared(searchSource(query))).results);
  /// The query the results on screen answer: the typed one, or its correction.
  const answered = $derived.by(() => {
    const typed = search.settledQuery;
    if (!search.results || !typed) return typed;
    return peek(searchSource(typed))?.query ?? typed;
  });
  const corrected = $derived(answered !== search.settledQuery ? answered : null);
  const history = searchHistory();

  /// The results' type filter: "all" is the overview (top result, tracks,
  /// rows); a type shows it alone, paged. It survives a new query, like a
  /// tab, and resets when the search is cleared.
  let kind = $state<SearchKind | "all">("all");
  const pages = createSearchPages((query, type, offset) => api.searchType(query, type, offset));
  const KIND_LABELS: Record<SearchKind | "all", MessageKey> = {
    all: "search.kind.all",
    tracks: "route.tracks",
    albums: "route.albums",
    artists: "route.profiles",
    playlists: "route.playlists",
  };
  const kinds = $derived(search.results ? kindsWithResults(search.results) : []);
  /// A type the current results lack falls back to the overview.
  const shownKind = $derived(kind !== "all" && kinds.includes(kind) ? kind : "all");

  $effect(() => {
    // Paging (type chips, Load more) asks for the corrected query's pages.
    const results = search.results;
    const query = answered;
    if (results) untrack(() => pages.seed(results, query));
  });

  /// The search in the route (`q`, `kind`) and the one on screen follow each
  /// other: a settled search or a chip writes the route in place (no new
  /// history entry per keystroke), and a route that differs from what was
  /// last written (mount, Back, the sidebar's plain Explore) is applied.
  let written: string | null = null;
  const routeKey = (q: string, k: SearchKind | "all") => `${q}\n${k}`;

  function applyRoute(route: Route) {
    if (route.name !== "explore") return;
    const q = route.q?.trim() ?? "";
    const k = route.kind ?? "all";
    written = routeKey(q, k);
    kind = k;
    if (q === search.settledQuery && search.results) return;
    if (!q) {
      search.reset();
      return;
    }
    const cached = peek(searchSource(q));
    if (cached) search.restore(q, cached.results);
    else {
      search.setQuery(q);
      search.runNow();
    }
  }

  // At once, so a restored search is on the first frame (and the scroll
  // position Back restores has its results to land on).
  applyRoute(untrack(() => nav.route));

  $effect(() => {
    const route = nav.route;
    untrack(() => {
      if (route.name !== "explore") return;
      const key = routeKey(route.q?.trim() ?? "", route.kind ?? "all");
      if (key !== written) applyRoute(route);
    });
  });

  $effect(() => {
    // Mid-search the old route stands; it moves once the results land.
    if (search.loading) return;
    const q = search.query.trim() ? search.settledQuery : "";
    const k = q ? kind : "all";
    untrack(() => {
      const key = routeKey(q, k);
      if (key === written || nav.route.name !== "explore") return;
      written = key;
      nav.replace(q ? (k === "all" ? { name: "explore", q } : { name: "explore", q, kind: k }) : { name: "explore" });
    });
  });

  // Both parts read through the page cache (lib/viewCache.ts); the personal
  // one changes with each play, so it always revalidates in the background.
  const exploreSource = { key: "explore:rows", fetch: () => api.getExplore() };
  const personalSource = {
    key: "explore:personal",
    fetch: async () => {
      const listened = (await api.getHistoryPage(0, 200)).items;
      const fresh = await buildNewToYou(listened, api, (id) => favorites.hasArtist(id)).catch(() => []);
      return { listened, newToYou: fresh };
    },
    freshMs: 0,
  };
  const cachedRows = peek(exploreSource);
  const cachedPersonal = peek(personalSource);

  let sections = $state<PageSection[]>(cachedRows ?? []);
  let exploreLoading = $state(cachedRows === undefined);
  let exploreError = $state("");

  async function loadExplore() {
    if (peek(exploreSource) === undefined) exploreLoading = true;
    exploreError = "";
    try {
      await loadCached(exploreSource, (rows) => (sections = rows));
    } catch (e) {
      exploreError = String(e);
    } finally {
      exploreLoading = false;
    }
  }

  /// Personal parts (chips, New to you) load on their own and stay hidden on
  /// failure or an empty history, like Home's shelves.
  let listened = $state<HistoryEntry[]>(cachedPersonal?.listened ?? []);
  let newToYou = $state<NewArtist[]>(cachedPersonal?.newToYou ?? []);

  async function loadPersonal() {
    try {
      await loadCached(personalSource, (value) => {
        listened = value.listened;
        newToYou = value.newToYou;
        personalSettled = true;
      });
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
  let personalSettled = $state(cachedPersonal !== undefined);
  let capped = $state(false);
  const ready = $derived((!exploreLoading && personalSettled) || capped);

  $effect(() => {
    // Untracked: cached sources apply synchronously; their reads must not
    // make this mount effect re-run on its own writes.
    untrack(() => {
      loadExplore();
      void loadPersonal();
    });
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
      artwork: artist.picture ?? artist.cover_fallback ?? null,
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
    search.setQuery(value);
    if (value.trim().length === 0) kind = "all";
  }

  /// A submitted search (Enter, a recent search, a chip) runs at once,
  /// without the typing debounce. `setQuery` re-arms the debounce, so an
  /// unchanged query (plain Enter) skips it, or the search would run twice.
  function runSearch(value: string) {
    if (value !== search.query) search.setQuery(value);
    search.runNow();
  }

</script>

<!-- Capped and centred, so a wide window doesn't pull the columns apart; the
     title, the browse rows and the results share the one edge. -->
<div class="explore">
<header class="page-head explore-head" class:searching>
<h2>{t("route.explore")}</h2>
<div class="search-block">
<SearchField
  value={search.query}
  oninput={onSearchInput}
  onsearch={runSearch}
  busy={search.loading && search.results !== null}
/>
{#if ready && !searching && chips.length > 0}
  <div class="chips" role="group" aria-label={t("search.suggestions")}>
    {#each chips as chip (chip.value)}
      <button
        type="button"
        class="chip"
        onclick={() => {
          history.record(chip.value);
          runSearch(chip.value);
        }}
        title={chip.recent ? t("search.again", { query: chip.value }) : t("search.for", { query: chip.value })}
      >
        <Icon name={chip.recent ? "history" : "user"} size={13} />
        <span>{chip.value}</span>
      </button>
    {/each}
  </div>
{/if}
{#if searching && kinds.length > 1}
  <!-- The results' type filter, in the suggestion chips' place. -->
  <div class="chips kinds" role="group" aria-label={t("search.showResults")}>
    {#each ["all", ...kinds] as const as option (option)}
      <button
        type="button"
        class="chip"
        aria-pressed={shownKind === option}
        onclick={() => (kind = option)}
      >{t(KIND_LABELS[option])}</button>
    {/each}
  </div>
{/if}
{#if searching && corrected && search.results}
  {@const [before, after] = tSplit("search.corrected", "query")}
  <p class="corrected" role="status">{before}<strong>{corrected}</strong>{after}</p>
{/if}
</div>
</header>

{#if searching}
  {#if search.loading && !search.results && !search.error}
    <Loading label={t("search.searching")} />
  {:else if search.error}
    <ErrorState message={search.error} onRetry={() => search.retry()} />
  {:else if noResults}
    <EmptyState message={t("search.noResults", { query: search.settledQuery })} />
  {:else if search.results}
    <SearchResultsView
      results={search.results}
      query={answered}
      {pages}
      kind={shownKind}
      onKind={(next) => (kind = next)}
      {nav}
      {player}
      {favorites}
      {playlistActions}
      {toasts}
    />
  {/if}
{:else if !ready}
  <div class="pending"><Loading /></div>
{:else}
  {#if newItems.length > 0}
    <section class="shelf reveal">
      <Carousel title={t("explore.newToYou")}>
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
    <EmptyState message={t("explore.empty")} />
  {:else}
    <div class="reveal later">
      <PageSections sections={[...arranged.quick, ...arranged.rest]} {nav} {player} {toasts} source="explore" />
    </div>
  {/if}
{/if}
</div>

<style>
  .explore {
    max-width: 1680px;
    margin-inline: auto;
  }
  /* Explore's header is its search: the title keeps every page's h2 step,
     the field below it is the page's hero (spec 2026-10-16 Explore). */
  .explore-head {
    flex-direction: column;
    flex-wrap: nowrap;
    align-items: stretch;
    gap: 12px;
    margin-bottom: 26px;
  }
  /* Results open with their own headings' margin; the chips sit closer. */
  .explore-head.searching {
    margin-bottom: 4px;
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
  /* The results' type filter: the same chips, the chosen one filled with
     the accent. */
  .kinds .chip {
    padding: 0 14px;
  }
  /* A corrected search says what it searched for, under the chips. */
  .corrected {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
    text-align: center;
  }
  .corrected strong {
    color: var(--text);
    font-weight: 600;
  }
  .kinds .chip[aria-pressed="true"] {
    border-color: transparent;
    background: var(--accent);
    color: var(--on-accent);
  }
</style>
