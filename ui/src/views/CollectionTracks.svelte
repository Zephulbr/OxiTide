<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import { createTrackView, visibleIndexOf } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";

  let {
    player,
    favorites,
    playlistActions,
  }: { player: Player; favorites: Favorites; playlistActions: PlaylistActions } = $props();

  const favoritesPager = createTrackPager("favorites", (offset, limit) =>
    api.getFavoriteTracksPage(offset, limit),
  );

  async function ensureAllFavorites() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) throw favoritesPager.error;
  }

  const view = createTrackView(() => favoritesPager.items, ensureAllFavorites);

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
    playSource.set(null);
    api.playTracks(view.items.map(trackToQueueItem), start);
  }

  async function playAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    playSource.set(null);
    api.playTracks(view.items.map(trackToQueueItem), 0);
  }

  async function shuffleAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    playSource.set(null);
    api.playTracks(view.items.map(trackToQueueItem), 0, true);
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
  <p>
    <button onclick={() => favoritesPager.refresh()}>Refresh</button>
    <span class="muted">
      {#if favoritesPager.unavailable > 0}
        {favoritesPager.items.length} tracks ({favoritesPager.unavailable} unavailable)
      {:else}
        {favoritesPager.items.length} tracks
      {/if}
    </span>
    {#if favoritesPager.loading}<Loading />{/if}
  </p>
  <TrackTable
    {view}
    {favorites}
    fill
    onPlay={playFrom}
    onEndReached={() => void favoritesPager.loadMore()}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    playingTrackId={player.state.current?.track_id ?? null}
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
  .list-head h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
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
</style>
