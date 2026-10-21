<script lang="ts">
  import { api, trackToQueueItem, type PageItem, type SearchKind, type SearchResults, type Track } from "../api";
  import type { Favorites } from "../favorites.svelte";
  import { routeForPageItem, type Nav } from "../nav.svelte";
  import type { PlaylistActions } from "../playlists.svelte";
  import type { Player } from "../player.svelte";
  import { playSource } from "../playSource.svelte";
  import type { SearchPages } from "../searchPages.svelte";
  import {
    albumCard,
    artistCard,
    artistRow,
    pickTopHit,
    playlistCard,
    shelfOrder,
    type ShelfKind,
  } from "../searchResults";
  import type { Toasts } from "../toasts.svelte";
  import { trackFlags } from "../trackFlags.svelte";
  import { createTrackView } from "../trackView.svelte";
  import Carousel from "./Carousel.svelte";
  import EmptyState from "./EmptyState.svelte";
  import ErrorState from "./ErrorState.svelte";
  import Icon from "./Icon.svelte";
  import MediaCard from "./MediaCard.svelte";
  import SearchTopHit from "./SearchTopHit.svelte";
  import TrackTable from "./TrackTable.svelte";

  let {
    results,
    query,
    pages,
    kind,
    onKind,
    nav,
    player,
    favorites,
    playlistActions,
    toasts,
  }: {
    results: SearchResults;
    /// The query these results answer.
    query: string;
    pages: SearchPages;
    /// "all" is the overview; a type shows that type alone, paged.
    kind: SearchKind | "all";
    onKind: (kind: SearchKind | "all") => void;
    nav: Nav;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    toasts: Toasts;
  } = $props();

  const top = $derived(pickTopHit(results, query));
  /// Beside a top result the tracks match its height; alone they get a
  /// little more room. Either way the rest is one click away ("See all").
  const previewCount = $derived(top ? 5 : 8);
  const allTracks = $derived(pages.items("tracks"));
  const preview = createTrackView(() => allTracks.slice(0, previewCount));
  const full = createTrackView(() => allTracks);

  const shelves = $derived(shelfOrder(top));
  const SHELF_TITLES: Record<ShelfKind, string> = { artists: "Artists", albums: "Albums", playlists: "Playlists" };
  const cards = $derived({
    artists: artistRow(results.artists.items).map(artistCard),
    albums: results.albums.items.map(albumCard),
    playlists: results.playlists.items.map(playlistCard),
  });

  /// The queue is every loaded result track, starting at the clicked one, so
  /// playing from the short preview keeps going past it.
  async function playFrom(track: Track | undefined, shuffle = false) {
    if (allTracks.length === 0) return;
    const start = track ? Math.max(0, allTracks.indexOf(track)) : 0;
    const prepared = await trackFlags.filterPlayable(allTracks, start);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), shuffle ? 0 : prepared.start, shuffle);
  }

  function open(item: PageItem): (() => void) | undefined {
    const route = routeForPageItem(item);
    return route ? () => nav.goto(route) : undefined;
  }

  const playing = $derived(player.state.current?.track_id ?? null);
</script>

{#snippet trackActions()}
  <button
    class="list-action"
    onclick={() => playFrom(undefined, true)}
    aria-label="Shuffle"
    title="Shuffle"
  ><Icon name="shuffle" size={16} /></button>
  <button
    class="list-action"
    onclick={() => playFrom(undefined)}
    aria-label="Play all"
    title="Play all"
  ><Icon name="play" size={16} /></button>
{/snippet}

{#snippet more(type: SearchKind)}
  {#if pages.error(type)}
    <ErrorState message={pages.error(type)} onRetry={() => void pages.more(type)} />
  {:else if pages.hasMore(type)}
    <div class="more">
      <button class="tool-btn" onclick={() => void pages.more(type)} disabled={pages.loading(type)}>
        {pages.loading(type) ? "Loading…" : "Load more"}
      </button>
    </div>
  {/if}
{/snippet}

<div class="results">
{#if kind === "all"}
  <div class="overview-frame">
  <div class="overview" class:with-top={top !== null}>
    {#if top}
      <section class="top" aria-labelledby="search-top">
        <h3 id="search-top">Top result</h3>
        <SearchTopHit hit={top} {nav} {toasts} />
      </section>
    {/if}
    {#if allTracks.length > 0}
      <section class="tracks" aria-labelledby="search-tracks">
        <div class="list-head">
          <h3 id="search-tracks">Tracks</h3>
          {#if allTracks.length > previewCount}
            <button type="button" class="view-all" aria-label="See all tracks" onclick={() => onKind("tracks")}>See all</button>
          {/if}
          {@render trackActions()}
        </div>
        <TrackTable
          view={preview}
          {nav}
          {favorites}
          {toasts}
          maxHeight="none"
          compact
          onPlay={(index) => playFrom(preview.items[index])}
          onAddToPlaylist={(t) => playlistActions.openPicker([t])}
          playingTrackId={playing}
          playbackRunning={player.state.playing}
        />
      </section>
    {/if}
  </div>
  </div>

  {#each shelves as shelf (shelf)}
    {#if cards[shelf].length > 0}
      <section class="block">
        <Carousel title={SHELF_TITLES[shelf]}>
          {#snippet controls()}
            <button type="button" class="view-all" aria-label={`See all ${shelf}`} onclick={() => onKind(shelf)}>See all</button>
          {/snippet}
          {#each cards[shelf] as item (item.id)}
            <MediaCard {item} variant="carousel" onclick={open(item)} />
          {/each}
        </Carousel>
      </section>
    {/if}
  {/each}
{:else if kind === "tracks"}
  {#if allTracks.length === 0}
    <EmptyState message={`No tracks for “${query}”`} />
  {:else}
    <div class="list-head">
      <h3>Tracks</h3>
      {@render trackActions()}
    </div>
    <TrackTable
      view={full}
      {nav}
      {favorites}
      {toasts}
      maxHeight="none"
      onPlay={(index) => playFrom(full.items[index])}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
      playingTrackId={playing}
      playbackRunning={player.state.playing}
    />
    {@render more("tracks")}
  {/if}
{:else}
  {@const items =
    kind === "artists"
      ? pages.items("artists").map(artistCard)
      : kind === "albums"
        ? pages.items("albums").map(albumCard)
        : pages.items("playlists").map(playlistCard)}
  {#if items.length === 0}
    <EmptyState message={`No ${kind} for “${query}”`} />
  {:else}
    <div class="grid pane">
      {#each items as item (item.id)}
        <MediaCard {item} variant="tile" onclick={open(item)} />
      {/each}
    </div>
    {@render more(kind)}
  {/if}
{/if}
</div>

<style>
  /* The top result and the first tracks side by side; stacked once the
     column is too narrow for both. A container query on the frame (an
     element cannot query itself), so the queue glide sees the final layout. */
  .overview-frame {
    container-type: inline-size;
    margin-bottom: 28px;
  }
  .overview {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 8px 48px;
  }
  .overview > * {
    min-width: 0;
  }
  @container (min-width: 820px) {
    .overview.with-top {
      grid-template-columns: 380px minmax(0, 1fr);
    }
  }
  .top {
    display: flex;
    flex-direction: column;
  }
  .top :global(.top-hit) {
    flex: 1;
  }
  .block {
    margin-bottom: 24px;
  }
  .list-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  /* As in the carousels' heads: the heading takes the row, and "See all"
     sits at its end before the play buttons. */
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
  .view-all {
    margin-right: 6px;
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .view-all:hover {
    color: var(--text);
  }
  /* A type on its own: the collection-grid recipe. */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
    gap: 18px 14px;
  }
  .more {
    display: flex;
    justify-content: center;
    margin: 20px 0 8px;
  }
</style>
