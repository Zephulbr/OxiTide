<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Track } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import { perf } from "../lib/perf.svelte";
  import { makeSyntheticTracks } from "../lib/perfFixture";
  import { perfHooks } from "../lib/perfHooks";
  import { playSource } from "../lib/playSource.svelte";
  import { trackFlags } from "../lib/trackFlags.svelte";
  import { createTrackView, visibleIndexOf, warmTrackSearch } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";
  import type { Nav } from "../lib/nav.svelte";

  let {
    nav,
    player,
    favorites,
    playlistActions,
    toasts,
  }: { nav: Nav; player: Player; favorites: Favorites; playlistActions: PlaylistActions; toasts: Toasts } = $props();

  const UNAVAILABLE_NOTE =
    "TIDAL still counts these favorites but no longer serves them: they were removed from the catalogue or aren't licensed in your region.";

  const favoritesPager = createTrackPager("favorites", (offset, limit) =>
    api.getFavoriteTracksPage(offset, limit),
  );

  /// S6-only: the perf bridge swaps in a synthetic 10k list so filter/sort
  /// budgets are measured at the spec's scale. Null in every normal run.
  let synthetic = $state<Track[] | null>(null);
  const rows = $derived(synthetic ?? favoritesPager.items);

  async function ensureAllFavorites() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) throw favoritesPager.error;
  }

  const view = createTrackView(
    () => rows,
    async () => {
      if (synthetic) return; // the synthetic fixture is already complete
      await ensureAllFavorites();
    },
    { memoryKey: "favorites" },
  );

  // Warm the filter/sort caches in idle chunks: a first keystroke over a 10k
  // list must not pay the whole build (~80 ms measured).
  $effect(() => {
    const list = rows;
    if (list.length < 500) return;
    return warmTrackSearch(list).cancel;
  });

  // Page-open gate: first painted page after the first window resolves, once
  // per run, real favorites only (the synthetic run has its own gates).
  let pageTimed = false;
  $effect(() => {
    if (pageTimed || synthetic !== null || rows.length === 0 || !perf.active) return;
    pageTimed = true;
    const started = performance.now();
    requestAnimationFrame(() =>
      perf.recordInteraction("page", performance.now() - started),
    );
  });

  $effect(() => {
    if (!perf.active) return;
    perfHooks.loadSyntheticTracks = (count: number) => {
      synthetic = makeSyntheticTracks(count);
    };
    perfHooks.toggleViewSort = (key) => view.toggleSort(key);
    return () => {
      delete perfHooks.loadSyntheticTracks;
      delete perfHooks.toggleViewSort;
    };
  });

  $effect(() => {
    untrack(() => favoritesPager.loadMore());
  });

  $effect(() => {
    favorites.trackRevision;
    const loaded = new Set(favoritesPager.items.map((t) => t.id));
    for (const id of favorites.pendingRemovalsToDrop(loaded)) favoritesPager.removeItem(id);
  });

  async function playFrom(index: number) {
    const clicked = view.items[index];
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    const start = visibleIndexOf(view.items, clicked, index);
    if (start < 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, start);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function playAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function shuffleAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), 0, true);
  }
</script>

<div class="view-fill">
  <div class="list-head">
    <h2>Tracks</h2>
    <button
      class="list-action"
      onclick={shuffleAll}
      disabled={view.items.length === 0}
      aria-label="Shuffle"
      title="Shuffle"
    ><Icon name="shuffle" size={16} /></button>
    <button
      class="list-action"
      onclick={playAll}
      disabled={view.items.length === 0}
      aria-label="Play all"
      title="Play all"
    ><Icon name="play" size={16} /></button>
  </div>
  <!-- The count, then Refresh as an icon that spins while a load runs.
       TIDAL counts unavailable favorites but never returns them, so they
       can't be listed; the button explains instead. -->
  <p class="list-meta">
    <span class="muted">{rows.length} tracks</span>
    {#if favoritesPager.unavailable > 0}
      <button
        class="unavailable"
        onclick={() => toasts.show(UNAVAILABLE_NOTE, { severity: "info" })}
        title={UNAVAILABLE_NOTE}
      ><Icon name="info" size={13} />{favoritesPager.unavailable} unavailable</button>
    {/if}
    <button
      class="refresh"
      class:spinning={favoritesPager.loading}
      onclick={() => favoritesPager.refresh()}
      aria-label={favoritesPager.loading ? "Refreshing" : "Refresh"}
      title="Refresh"
    ><Icon name="refresh-cw" size={14} /></button>
  </p>
  <TrackTable
    {view}
    {nav}
    {favorites}
    {toasts}
    fill
    kind="favorites"
    onPlay={playFrom}
    onEndReached={() => void favoritesPager.loadMore()}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    playingTrackId={player.state.current?.track_id ?? null}
    playbackRunning={player.state.playing}
  />
  {#if favoritesPager.error}
    <ErrorState message={favoritesPager.error} onRetry={() => favoritesPager.loadMore()} />
  {/if}
</div>

<style>
  .list-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  /* Not a .page-head, so it takes the page-title size itself (app.css). */
  .list-head h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: var(--page-title-size, 32px);
    line-height: 1.1;
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
  .list-meta {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .unavailable {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 999px;
    color: var(--text-muted);
  }
  .unavailable:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .refresh {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    color: var(--text-muted);
  }
  .refresh:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .refresh.spinning :global(svg) {
    animation: refresh-spin 0.9s linear infinite;
  }
  @keyframes refresh-spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .refresh.spinning :global(svg) {
      animation: none;
    }
  }
</style>
