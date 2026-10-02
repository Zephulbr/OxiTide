<script lang="ts">
  import { api, trackToQueueItem, type Album as AlbumT, type Track } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import DetailAction from "../lib/components/DetailAction.svelte";
  import DetailHeader from "../lib/components/DetailHeader.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { copyText } from "../lib/clipboard";
  import { formatTotalDuration } from "../lib/format";
  import { perf } from "../lib/perf.svelte";
  import { makeSyntheticAlbum } from "../lib/perfFixture";
  import { perfHooks } from "../lib/perfHooks";
  import { createTrackView } from "../lib/trackView.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import { shareUrl } from "../lib/share";
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

  // "12 TRACKS (43:21)" — album totals with loaded-item fallback.
  const stats = $derived.by(() => {
    if (!album) return "";
    const count = album.numberOfTracks || tracks.length;
    const seconds = album.duration || tracks.reduce((sum, t) => sum + t.duration, 0);
    const label = `${count} ${count === 1 ? "TRACK" : "TRACKS"}`;
    return seconds > 0 ? `${label} (${formatTotalDuration(seconds)})` : label;
  });

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

  async function share() {
    const copied = await copyText(shareUrl("album", id));
    toasts.show(copied ? "Link copied" : "Couldn't copy link", {
      severity: copied ? "success" : "error",
    });
  }
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if album}
  {@const albumId = album.id}
  {@const data = album}
  <div class="view-fill">
    <DetailHeader
      kicker="Album"
      title={data.title}
      subtitle={data.artists.map((a) => a.name).join(", ")}
      {stats}
      onPlay={playAll}
      onShuffle={shuffleAll}
      disabled={view.items.length === 0}
    >
      {#snippet cover()}
        <Cover uuid={data.cover} size={640} radius={12} alt={`${data.title} cover`} />
      {/snippet}
      {#snippet actions()}
        <DetailAction
          icon="heart"
          label={favorites.hasAlbum(albumId) ? "Favorited" : "Favorite"}
          active={favorites.hasAlbum(albumId)}
          title={favorites.hasAlbum(albumId) ? "Unfavorite album" : "Favorite album"}
          onclick={() => favorites.toggleAlbum(albumId)}
        />
        <DetailAction
          icon="list-plus"
          label="Add"
          title="Add album to playlist"
          onclick={() => playlistActions.openPicker(tracks)}
        />
        <DetailAction icon="share-2" label="Share" title="Copy link" onclick={share} />
      {/snippet}
    </DetailHeader>
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
