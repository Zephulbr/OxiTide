<script lang="ts">
  import { api, trackToQueueItem, type Album as AlbumT, type AlbumInfo, type Track } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import DetailAction from "../lib/components/DetailAction.svelte";
  import DetailHeader from "../lib/components/DetailHeader.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { volumeSections } from "../lib/albumVolumes";
  import { copyText } from "../lib/clipboard";
  import { formatTotalDuration } from "../lib/format";
  import { perf } from "../lib/perf.svelte";
  import { makeSyntheticAlbum } from "../lib/perfFixture";
  import { perfHooks } from "../lib/perfHooks";
  import { createTrackView } from "../lib/trackView.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import { trackFlags } from "../lib/trackFlags.svelte";
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
  let info = $state<AlbumInfo | null>(null);
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

  // Release facts from the public v2 album; the public payload has no label,
  // so the row shows what exists and omits the rest (spec §6.2).
  const facts = $derived.by(() => {
    const rows: { label: string; value: string }[] = [];
    if (info?.releaseDate) rows.push({ label: "Released", value: info.releaseDate });
    if (info?.copyright) rows.push({ label: "Copyright", value: info.copyright });
    if (info?.barcodeId) rows.push({ label: "UPC", value: info.barcodeId });
    if (info && info.numberOfVolumes && info.numberOfVolumes > 1) {
      rows.push({ label: "Volumes", value: String(info.numberOfVolumes) });
    }
    return rows;
  });
  const volumes = $derived(volumeSections(tracks));

  async function load() {
    loading = true;
    error = "";
    try {
      const [loadedAlbum, loadedTracks, loadedInfo] = await Promise.all([
        api.getAlbum(id),
        api.getAlbumTracks(id),
        api.getAlbumInfo(id).catch(() => null),
      ]);
      album = loadedAlbum;
      tracks = loadedTracks;
      info = loadedInfo;
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
      info = null;
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

  async function playFrom(index: number) {
    const prepared = await trackFlags.filterPlayable(view.items, index);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function playAll() {
    if (view.items.length === 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function shuffleAll() {
    if (view.items.length === 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), 0, true);
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
    {#if facts.length > 0}
      <dl class="facts">
        {#each facts as fact (fact.label)}
          <div>
            <dt>{fact.label}</dt>
            <dd>{fact.value}</dd>
          </div>
        {/each}
      </dl>
    {/if}
    <TrackTable
      {view}
      {favorites}
      {toasts}
      fill
      showAlbum={false}
      sections={volumes ?? undefined}
      onPlay={playFrom}
      playingTrackId={player.state.current?.track_id ?? null}
      playbackRunning={player.state.playing}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    />
  </div>
{:else}
  <EmptyState message="Album not found." />
{/if}

<style>
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 28px;
    margin: -24px 0 24px;
    padding: 0;
  }
  .facts dt {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .facts dd {
    margin: 2px 0 0;
    font-size: 13.5px;
    overflow-wrap: anywhere;
  }
</style>
