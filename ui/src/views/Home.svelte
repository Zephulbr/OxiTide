<script lang="ts">
  import { untrack } from "svelte";
  import {
    api,
    type Artist,
    type FeedActivity,
    type PageItem,
    type PageSection,
    type Stats,
  } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { editorialSections, feedAlbumItems, madeForYou, unseenCount } from "../lib/homeData";
  import { buildSuggestions, type HomeSuggestions, type RadioSeed } from "../lib/homeSuggest";
  import { shortcutEntries, type ShortcutEntry } from "../lib/media";
  import { routeForPageItem, type Nav } from "../lib/nav.svelte";
  import { playlistRecency } from "../lib/playlistRecency.svelte";
  import type { Player } from "../lib/player.svelte";
  import { playArtistRadio, playTrackRadio } from "../lib/radio";
  import type { Toasts } from "../lib/toasts.svelte";
  import { load as loadCached, peek, type Source } from "../lib/viewCache";
  import { feedSource, forYouSource } from "../lib/viewData";
  import Carousel from "../lib/components/Carousel.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import HomeHero from "../lib/components/HomeHero.svelte";
  import HomeListening from "../lib/components/HomeListening.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import PageSections from "../lib/components/PageSections.svelte";

  let { nav, player, toasts }: { nav: Nav; player: Player; toasts: Toasts } = $props();

  // Every source reads through the page cache (lib/viewCache.ts). The
  // personal ones change with each play, so they always revalidate in the
  // background (`freshMs: 0`); the cache only removes the wait.
  const tz = new Date().getTimezoneOffset();
  const sources = {
    editorial: { key: "home:editorial", fetch: () => api.getHome() },
    shortcuts: {
      key: "home:shortcuts",
      fetch: async () => {
        const [playlists, mixes] = await Promise.allSettled([api.getUserPlaylists(), api.getFavoriteMixes()]);
        return {
          playlists: playlists.status === "fulfilled" ? playlists.value : [],
          mixes: mixes.status === "fulfilled" ? mixes.value : [],
        };
      },
      freshMs: 0,
    },
    // Shared with the Mixes and Feed pages (lib/viewData.ts).
    forYou: forYouSource(),
    feed: feedSource(),
    stats: { key: "home:stats", fetch: () => api.getStats(tz), freshMs: 0 },
    suggestions: {
      key: "home:suggestions",
      fetch: async () => buildSuggestions((await api.getHistoryPage(0, 60)).items, api),
      freshMs: 0,
    },
  } satisfies Record<string, Source<unknown>>;

  const cachedEditorial = peek(sources.editorial);
  let editorial = $state<PageSection[]>(cachedEditorial ?? []);
  let loading = $state(cachedEditorial === undefined);
  let error = $state("");

  let shortcuts = $state<ShortcutEntry[]>([]);
  let suggestions = $state<HomeSuggestions | null>(null);
  let mixes = $state<PageItem[]>([]);
  let releases = $state<FeedActivity[]>([]);
  let stats = $state<Stats | null>(null);

  const releaseItems = $derived(feedAlbumItems(releases));
  const unseen = $derived(unseenCount(releases));
  const trimmed = $derived(editorialSections(editorial));

  /// Recently played albums, as album cards.
  const albumItems = $derived<PageItem[]>(
    (suggestions?.albums ?? []).slice(0, 12).map((group) => ({
      id: String(group.albumId),
      title: group.album,
      subtitle: group.artistName,
      item_type: "ALBUM",
      artwork: group.cover,
      artist_refs: [{ id: group.artistId, name: group.artistName }],
    })),
  );
  const radios = $derived(suggestions?.radios ?? []);
  const similar = $derived<PageItem[]>(
    (suggestions?.similar ?? []).slice(0, 12).map((artist: Artist) => ({
      id: String(artist.id),
      title: artist.name,
      subtitle: "Artist",
      item_type: "ARTIST",
      artwork: artist.picture ?? artist.cover_fallback ?? null,
    })),
  );

  async function load() {
    if (peek(sources.editorial) === undefined) loading = true;
    error = "";
    try {
      await loadCached(sources.editorial, (sections) => (editorial = sections));
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  /// Shelves reveal together once the personal sources have settled (or after
  /// REVEAL_CAP_MS, whatever has arrived by then). Shown on arrival, a late
  /// shelf (suggestions resolve in a second round) inserted itself above ones
  /// already on screen and shoved the rest of Home down. TIDAL's editorial
  /// rows sit below them all, so they wait too. A revisit with every personal
  /// source cached reveals at once.
  const REVEAL_CAP_MS = 2500;
  const personalSources = [sources.shortcuts, sources.forYou, sources.feed, sources.stats, sources.suggestions];
  let personalSettled = $state(false);
  let capped = $state(personalSources.every((source) => peek(source as Source<unknown>) !== undefined));
  const ready = $derived(personalSettled || capped);

  /// One personal source: settles on its first value (cached or fetched) or
  /// on failure; a failed source leaves its shelf hidden.
  function part<T>(source: Source<T>, apply: (value: T) => void): Promise<void> {
    return new Promise((resolve) => {
      loadCached(source, (value) => {
        apply(value);
        resolve();
      })
        .catch(() => {})
        .finally(resolve);
    });
  }

  /// Every personal shelf loads on its own and stays hidden when its source
  /// fails or is empty, so a new account or a flaky endpoint never blanks Home.
  async function loadPersonal() {
    const recency = playlistRecency();
    await Promise.all([
      part(sources.shortcuts, ({ playlists, mixes: favMixes }) => {
        const owned = [...playlists].sort((a, b) => recency.rank(b.uuid) - recency.rank(a.uuid));
        shortcuts = shortcutEntries(owned, favMixes, 8);
      }),
      part(sources.forYou, (sections) => (mixes = madeForYou(sections))),
      part(sources.feed, (feed) => (releases = feed)),
      part(sources.stats, (value) => (stats = value)),
      part(sources.suggestions, (built) => (suggestions = built)),
    ]);
    personalSettled = true;
  }

  $effect(() => {
    // Untracked: cached sources apply synchronously; their reads must not
    // make this mount effect re-run on its own writes.
    untrack(() => {
      void load();
      void loadPersonal();
    });
    const timer = setTimeout(() => (capped = true), REVEAL_CAP_MS);
    return () => clearTimeout(timer);
  });
  /// A shelf that throws while rendering drops out like one whose source
  /// failed. The shelves reveal in one update, so without a boundary each,
  /// one bad shelf would abort the reveal and leave Home on its spinner.
  function shelfFailed(error: unknown) {
    console.error("Home shelf failed to render", error);
  }

  function open(item: PageItem): (() => void) | undefined {
    const route = routeForPageItem(item);
    return route ? () => nav.goto(route) : undefined;
  }

  function radioCard(seed: RadioSeed): PageItem {
    // `MIX_LIST` gives the card the radio badge; the click starts the radio
    // instead of routing to a mix page.
    return {
      id: seed.key,
      title: seed.title,
      subtitle: seed.subtitle,
      item_type: "MIX_LIST",
      artwork: seed.cover,
    };
  }

  function startRadio(seed: RadioSeed) {
    void (seed.kind === "artist"
      ? playArtistRadio(seed.id, toasts)
      : playTrackRadio(seed.id, toasts));
  }
</script>

<div class="home">
  <HomeHero {player} go={(route) => nav.goto(route)} onerror={(e) => toasts.show(errorMessage(e), { severity: "error" })} />

  {#if !ready}
    <div class="pending"><Loading /></div>
  {:else}
  <svelte:boundary onerror={shelfFailed}>
  {#if shortcuts.length > 0}
    <section class="block" aria-labelledby="home-jump">
      <h3 id="home-jump">Jump back in</h3>
      <div class="shortcuts pane">
        {#each shortcuts as entry (entry.key)}
          <div class="shortcut lit-tint">
            <MediaCard
              item={entry.item}
              variant="row"
              thumb={64}
              collage={entry.playlist}
              onclick={() => nav.goto(entry.route)}
            />
          </div>
        {/each}
      </div>
    </section>
  {/if}
  </svelte:boundary>

  <svelte:boundary onerror={shelfFailed}>
  {#if albumItems.length > 0}
    <section class="block">
      <Carousel title="Recently played">
        {#snippet controls()}
          <button type="button" class="view-all" onclick={() => nav.goto({ name: "history", tab: "recent" })}>
            History
          </button>
        {/snippet}
        {#each albumItems as item (item.id)}
          <MediaCard {item} variant="mini" onclick={open(item)} />
        {/each}
      </Carousel>
    </section>
  {/if}
  </svelte:boundary>

  <svelte:boundary onerror={shelfFailed}>
  {#if mixes.length > 0}
    <section class="block">
      <Carousel title="Made for you">
        {#snippet controls()}
          <button type="button" class="view-all" onclick={() => nav.goto({ name: "mixes" })}>See all</button>
        {/snippet}
        {#each mixes as item (item.id)}
          <MediaCard {item} variant="carousel" onclick={open(item)} />
        {/each}
      </Carousel>
    </section>
  {/if}
  </svelte:boundary>

  <svelte:boundary onerror={shelfFailed}>
  {#if releaseItems.length > 0}
    <section class="block">
      <Carousel title={unseen > 0 ? `New from artists you follow · ${unseen} new` : "New from artists you follow"}>
        {#snippet controls()}
          <button type="button" class="view-all" onclick={() => nav.goto({ name: "feed" })}>Open feed</button>
        {/snippet}
        {#each releaseItems as item (item.id)}
          <MediaCard {item} variant="carousel" onclick={open(item)} />
        {/each}
      </Carousel>
    </section>
  {/if}
  </svelte:boundary>

  <svelte:boundary onerror={shelfFailed}>
  {#if radios.length > 0}
    <section class="block" aria-labelledby="home-explore">
      <h3 id="home-explore">Keep exploring</h3>
      <p class="hint muted">Radio built from what you've been playing.</p>
      <div class="radios pane">
        {#each radios as seed (seed.key)}
          <div class="radio lit-tint">
            <MediaCard
              item={radioCard(seed)}
              variant="row"
              thumb={92}
              playButton
              onclick={() => startRadio(seed)}
            />
          </div>
        {/each}
      </div>
    </section>
  {/if}
  </svelte:boundary>

  <svelte:boundary onerror={shelfFailed}>
  {#if suggestions?.similarTo && similar.length > 0}
    <section class="block">
      <Carousel title={`Because you listen to ${suggestions.similarTo.name}`}>
        {#each similar as item (item.id)}
          <MediaCard {item} variant="carousel" onclick={open(item)} />
        {/each}
      </Carousel>
    </section>
  {/if}
  </svelte:boundary>

  <svelte:boundary onerror={shelfFailed}>
  {#if stats && stats.total_plays > 0}
    <section class="block">
      <h3>Your listening</h3>
      <HomeListening {stats} {nav} {toasts} />
    </section>
  {/if}
  </svelte:boundary>

  <section class="block editorial">
    {#if loading}
      <Loading />
    {:else if error}
      <ErrorState message={error} onRetry={load} />
    {:else if trimmed.length > 0}
      <div class="editorial-body">
        <p class="eyebrow">From TIDAL</p>
        <PageSections sections={trimmed} {nav} {player} {toasts} source="home" />
      </div>
    {/if}
  </section>
  {/if}
</div>

<style>
  .home {
    display: flex;
    flex-direction: column;
    padding-bottom: 16px;
  }
  .block {
    margin-bottom: 18px;
    /* Shelves rise in as one sequence when Home reveals them (`ready`). */
    animation: shelf-in var(--dur-view) var(--ease-enter) backwards;
  }
  .block:nth-child(3) { animation-delay: 40ms; }
  .block:nth-child(4) { animation-delay: 80ms; }
  .block:nth-child(5) { animation-delay: 120ms; }
  .block:nth-child(n + 6) { animation-delay: 160ms; }
  /* The editorial rows rise on their own body instead, whether they are
     ready at the reveal or finish loading after it. */
  .block.editorial {
    animation: none;
  }
  .editorial-body {
    animation: shelf-in var(--dur-view) var(--ease-enter) 160ms backwards;
  }
  @keyframes shelf-in {
    from {
      opacity: 0;
      transform: translateY(var(--rise));
    }
  }
  .pending {
    margin-bottom: 18px;
  }
  .block > h3 {
    margin: 0 0 10px;
  }
  .shortcuts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 10px 16px;
    container: shortcuts / inline-size;
  }
  /* One row only: hide the cards that would wrap. n cards (240px min, 16px
     gaps) need 256n - 16 px. This is CSS, not a measured count, so it resolves
     in the same layout pass as any width change — the queue glide measures
     the final layout without a late reflow (a JS ResizeObserver count landed a
     frame later and made the page below jump). Up to 8 cards (Home caps it). */
  @container shortcuts (width < 2032px) {
    .shortcut:nth-child(n + 8) {
      display: none;
    }
  }
  @container shortcuts (width < 1776px) {
    .shortcut:nth-child(n + 7) {
      display: none;
    }
  }
  @container shortcuts (width < 1520px) {
    .shortcut:nth-child(n + 6) {
      display: none;
    }
  }
  @container shortcuts (width < 1264px) {
    .shortcut:nth-child(n + 5) {
      display: none;
    }
  }
  @container shortcuts (width < 1008px) {
    .shortcut:nth-child(n + 4) {
      display: none;
    }
  }
  @container shortcuts (width < 752px) {
    .shortcut:nth-child(n + 3) {
      display: none;
    }
  }
  @container shortcuts (width < 496px) {
    .shortcut:nth-child(n + 2) {
      display: none;
    }
  }
  /* Containers: lit edge (`.lit`) plus a resting drop shadow, so they sit
     above the backdrop instead of printing flat onto it. */
  .shortcut {
    border-radius: var(--radius-control);
    box-shadow: var(--shadow-panel);
  }
  .hint {
    margin: -6px 0 12px;
    font-size: 13px;
  }
  /* Keep exploring: wide radio cards with big art, lit like panels. */
  .radios {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 14px;
  }
  .radio {
    padding: 6px;
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-panel);
  }
  .view-all {
    color: var(--text-muted);
    font-size: 12.5px;
    margin-left: 6px;
  }
  .view-all:hover {
    color: var(--text);
  }
  .editorial {
    margin-top: 10px;
  }
  .eyebrow {
    margin: 0 0 4px;
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
</style>
