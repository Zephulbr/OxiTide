<script lang="ts">
  import { api, trackToQueueItem, type MixDetail } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import { createTrackView } from "../lib/trackView.svelte";
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
  }: { id: string; player: Player; favorites: Favorites; playlistActions: PlaylistActions; toasts: Toasts } = $props();

  let mix = $state<MixDetail | null>(null);
  let loading = $state(true);
  let error = $state("");

  const view = createTrackView(() => mix?.tracks ?? []);
  const artwork = $derived(mix?.artwork ?? mix?.tracks[0]?.album?.cover ?? null);

  async function load() {
    loading = true;
    error = "";
    try {
      mix = await api.getMix(id);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });

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

  function playFrom(index: number) {
    playSource.set(null);
    void api.playTracks(view.items.map(trackToQueueItem), index);
  }
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if mix}
  <div class="view-fill">
    <header class="view-header">
      <span class="header-art"><Cover uuid={artwork} size={320} radius={12} alt={`${mix.title} cover`} /></span>
      <div class="header-meta">
        <h2>{mix.title}</h2>
        <p class="muted">Mix · {view.items.length} tracks</p>
        <div class="header-actions">
          <button
            onclick={() => playlistActions.openPicker(view.items)}
            aria-label="Add mix to playlist"
            title="Add mix to playlist"
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
  <EmptyState message="Mix not found." />
{/if}

<style>
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
