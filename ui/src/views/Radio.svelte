<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Track } from "../lib/api";
  import ArtistLinks from "../lib/components/ArtistLinks.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import DetailAction from "../lib/components/DetailAction.svelte";
  import DetailHeader from "../lib/components/DetailHeader.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { formatTotalDuration } from "../lib/format";
  import { playSource } from "../lib/playSource.svelte";
  import { radioErrorMessage, trackRadioName, trackRadioOrigin } from "../lib/radio";
  import { trackFlags } from "../lib/trackFlags.svelte";
  import { createTrackView } from "../lib/trackView.svelte";
  import { load as loadCached, peek } from "../lib/viewCache";
  import { radioSource } from "../lib/viewData";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";
  import type { Nav } from "../lib/nav.svelte";

  /// A track radio, laid out like a mix: the seed track's cover and artists
  /// in the header, the station below. Nothing plays until asked.
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
  const source = radioSource(id);
  const cached = peek(source);
  let data = $state<[Track, Track[]] | null>(cached ?? null);
  let loading = $state(cached === undefined);
  let error = $state("");

  const seed = $derived(data?.[0] ?? null);
  // svelte-ignore state_referenced_locally
  const view = createTrackView(() => data?.[1] ?? [], undefined, { memoryKey: `radio:${id}` });
  const origin = $derived(trackRadioOrigin(id, seed?.title));

  // "50 TRACKS (3:12:44)", summed like a mix.
  const stats = $derived.by(() => {
    const tracks = data?.[1] ?? [];
    const seconds = tracks.reduce((sum, t) => sum + t.duration, 0);
    const label = `${tracks.length} ${tracks.length === 1 ? "TRACK" : "TRACKS"}`;
    return seconds > 0 ? `${label} (${formatTotalDuration(seconds)})` : label;
  });

  async function load() {
    if (peek(source) === undefined) loading = true;
    error = "";
    try {
      await loadCached(source, (loaded) => (data = loaded));
    } catch (e) {
      error = radioErrorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    untrack(load);
  });

  async function playFrom(index: number, shuffle = false) {
    if (view.items.length === 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, index);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(origin, prepared.items.map((t) => t.id));
    void api.playTracks(prepared.items.map(trackToQueueItem), shuffle ? 0 : prepared.start, shuffle || undefined);
  }
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if seed && data}
  {@const track = seed}
  <div class="view-fill">
    <DetailHeader
      kicker="Track radio"
      title={trackRadioName(track.title)}
      {stats}
      onPlay={() => void playFrom(0)}
      onShuffle={() => void playFrom(0, true)}
      disabled={view.items.length === 0}
      drag={() => (view.items.length > 0 ? { kind: "tracks", tracks: view.items.slice() } : null)}
    >
      {#snippet byline()}
        <ArtistLinks artists={track.artists} onOpen={(artistId) => nav.goto({ name: "artist", id: artistId })} />
      {/snippet}
      {#snippet cover()}
        <Cover uuid={track.album?.cover} size={640} radius={12} alt={`${track.title} cover`} />
      {/snippet}
      {#snippet actions()}
        <DetailAction
          icon="list-plus"
          label="Add"
          title="Add radio to playlist"
          onclick={() => playlistActions.openPicker(view.items)}
        />
      {/snippet}
    </DetailHeader>
    {#if data[1].length === 0}
      <EmptyState message="No radio tracks available." />
    {:else}
      <TrackTable
        {view}
        {nav}
        {favorites}
        {toasts}
        fill
        showAlbum={false}
        kind="mix"
        onPlay={(index) => void playFrom(index)}
        playingTrackId={player.state.current?.track_id ?? null}
        playbackRunning={player.state.playing}
        onAddToPlaylist={(t) => playlistActions.openPicker([t])}
        onAddManyToPlaylist={(ts) => playlistActions.openPicker(ts)}
      />
    {/if}
  </div>
{:else}
  <EmptyState message="Radio not found." />
{/if}
