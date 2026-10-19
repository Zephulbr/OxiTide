<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Album as AlbumT, type Track } from "../lib/api";
  import ArtistLinks from "../lib/components/ArtistLinks.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import DetailAction from "../lib/components/DetailAction.svelte";
  import DetailHeader from "../lib/components/DetailHeader.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { volumeSections } from "../lib/albumVolumes";
  import { copyText } from "../lib/clipboard";
  import { formatReleaseDate, formatTotalDuration } from "../lib/format";
  import { isAlbumRelease } from "../lib/media";
  import { perf } from "../lib/perf.svelte";
  import { makeSyntheticAlbum } from "../lib/perfFixture";
  import { perfHooks } from "../lib/perfHooks";
  import { createTrackView } from "../lib/trackView.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import { trackFlags } from "../lib/trackFlags.svelte";
  import { shareUrl } from "../lib/share";
  import { load as loadCached, peek } from "../lib/viewCache";
  import { albumSource } from "../lib/viewData";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";
  import type { Nav } from "../lib/nav.svelte";
  let {
    id,
    nav,
    player,
    favorites,
    playlistActions,
    toasts,
  }: { id: number; nav: Nav; player: Player; favorites: Favorites; playlistActions: PlaylistActions; toasts: Toasts } = $props();
  // A cached visit renders on the first frame (lib/viewCache.ts).
  // svelte-ignore state_referenced_locally
  const source = albumSource(id);
  const cached = peek(source);
  let album = $state<AlbumT | null>(cached?.[0] ?? null);
  let tracks = $state<Track[]>(cached?.[1] ?? []);
  let loading = $state(cached === undefined);
  let error = $state("");

  // svelte-ignore state_referenced_locally
  const view = createTrackView(() => tracks, undefined, { memoryKey: `album:${id}` });

  // "FEB 11, 2014 · 12 TRACKS (43:21)" — release date (when known), then album
  // totals with loaded-item fallback. The stats line is the only place the
  // release date shows; copyright/UPC were dropped as noise.
  const stats = $derived.by(() => {
    if (!album) return "";
    const count = album.numberOfTracks || tracks.length;
    const seconds = album.duration || tracks.reduce((sum, t) => sum + t.duration, 0);
    const label = `${count} ${count === 1 ? "TRACK" : "TRACKS"}`;
    const totals = seconds > 0 ? `${label} (${formatTotalDuration(seconds)})` : label;
    const released = formatReleaseDate(album.releaseDate);
    return released ? `${released} · ${totals}` : totals;
  });
  const volumes = $derived(volumeSections(tracks));

  async function load() {
    if (peek(source) === undefined) loading = true;
    error = "";
    try {
      await loadCached(source, ([loadedAlbum, loadedTracks]) => {
        album = loadedAlbum;
        tracks = loadedTracks;
      });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    // Untracked: a cached load applies synchronously, and its reads must not
    // make this mount effect re-run on its own writes (an endless loop).
    untrack(load);
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
      {stats}
      onPlay={playAll}
      onShuffle={shuffleAll}
      disabled={view.items.length === 0}
    >
      {#snippet byline()}
        <ArtistLinks artists={data.artists} onOpen={(artistId) => nav.goto({ name: "artist", id: artistId })} />
      {/snippet}
      {#snippet cover()}
        <Cover
          uuid={data.cover}
          size={640}
          radius={isAlbumRelease(data.type) ? 0 : 12}
          alt={`${data.title} cover`}
        />
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
      {nav}
      {favorites}
      {toasts}
      fill
      showAlbum={false}
      kind="album"
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
