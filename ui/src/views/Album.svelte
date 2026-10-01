<script lang="ts">
  import { api, trackToQueueItem, type Album as AlbumT, type Track } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { perf } from "../lib/perf.svelte";
  import { makeSyntheticAlbum } from "../lib/perfFixture";
  import { perfHooks } from "../lib/perfHooks";
  import { createTrackView } from "../lib/trackView.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";
  let {
    id,
    player,
    favorites,
    playlistActions,
    toasts,
  }: { id: number; player: Player; favorites: Favorites; playlistActions: PlaylistActions; toasts: Toasts } = $props();
  let album = $state<AlbumT | null>(null);
  let tracks = $state<Track[]>([]);
  let loading = $state(true);
  let error = $state("");

  const view = createTrackView(() => tracks);

  async function load() {
    loading = true;
    error = "";
    try {
      [album, tracks] = await Promise.all([api.getAlbum(id), api.getAlbumTracks(id)]);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });

  $effect(() => {
    if (!perf.active) return;
    perfHooks.measureAlbumOpen = (count: number) => {
      const started = performance.now();
      const data = makeSyntheticAlbum(count);
      album = data.album;
      tracks = data.tracks;
      error = "";
      loading = false;
      requestAnimationFrame(() =>
        perf.recordInteraction("album", performance.now() - started),
      );
    };
    return () => {
      delete perfHooks.measureAlbumOpen;
    };
  });

  function playFrom(index: number) {
    playSource.set(null);
    void api.playTracks(view.items.map(trackToQueueItem), index);
  }

  function playAll() {
    if (view.items.length === 0) return;
    playSource.set(null);
    void api.playTracks(view.items.map(trackToQueueItem), 0);
  }

  function shuffleAll() {
    if (view.items.length === 0) return;
    playSource.set(null);
    void api.playTracks(view.items.map(trackToQueueItem), 0, true);
  }
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if album}
  {@const albumId = album.id}
  <div class="view-fill">
    <header class="view-header">
      <span class="header-art"><Cover uuid={album.cover} size={320} radius={12} alt={`${album.title} cover`} /></span>
      <div class="header-meta">
        <h2>{album.title}</h2>
        <p class="muted">{album.artists.map((a) => a.name).join(", ")} · {album.numberOfTracks} tracks</p>
        <div class="header-actions">
          <button
            class:on={favorites.hasAlbum(albumId)}
            onclick={() => favorites.toggleAlbum(albumId)}
            aria-label={favorites.hasAlbum(albumId) ? "Unfavorite album" : "Favorite album"}
            title={favorites.hasAlbum(albumId) ? "Unfavorite album" : "Favorite album"}
          ><Icon name="heart" /></button>
          <button
            onclick={() => playlistActions.openPicker(tracks)}
            aria-label="Add album to playlist"
            title="Add album to playlist"
          >Add to playlist</button>
        </div>
      </div>
    </header>
    <div class="header-buttons">
      <button class="btn-play" onclick={playAll} disabled={view.items.length === 0}>
        <Icon name="play" size={18} /> Play
      </button>
      <button class="btn-shuffle" onclick={shuffleAll} disabled={view.items.length === 0}>
        <Icon name="shuffle" size={18} /> Shuffle
      </button>
    </div>
    <TrackTable
      {view}
      {favorites}
      {toasts}
      fill
      showAlbum={false}
      onPlay={playFrom}
      playingTrackId={player.state.current?.track_id ?? null}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    />
  </div>
{:else}
  <EmptyState message="Album not found." />
{/if}

<style>
  .on { color: var(--accent); }
  .on :global(svg) { fill: currentColor; }
  .view-header {
    display: flex;
    align-items: flex-end;
    gap: 20px;
    margin-bottom: 14px;
  }
  .header-art {
    flex: 0 0 auto;
    width: 160px;
  }
  /* Big labeled actions under the artwork (screenshot layout, app tokens). */
  .header-buttons {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 18px;
  }
  .header-buttons button {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 22px;
    border-radius: var(--radius-control);
    font-weight: 600;
  }
  .btn-play {
    background: var(--accent);
    color: var(--on-accent);
  }
  .btn-play :global(svg) {
    fill: currentColor;
  }
  .btn-shuffle {
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .btn-shuffle:hover {
    color: var(--text);
  }
  .header-meta {
    min-width: 0;
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }
</style>
