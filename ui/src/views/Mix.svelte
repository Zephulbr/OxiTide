<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type MixDetail } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import DetailAction from "../lib/components/DetailAction.svelte";
  import DetailHeader from "../lib/components/DetailHeader.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { copyText } from "../lib/clipboard";
  import { formatTotalDuration } from "../lib/format";
  import { playSource } from "../lib/playSource.svelte";
  import { trackFlags } from "../lib/trackFlags.svelte";
  import { shareUrl } from "../lib/share";
  import { createTrackView } from "../lib/trackView.svelte";
  import { load as loadCached, peek } from "../lib/viewCache";
  import { mixSource } from "../lib/viewData";
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
  }: { id: string; nav: Nav; player: Player; favorites: Favorites; playlistActions: PlaylistActions; toasts: Toasts } = $props();

  // A cached visit renders on the first frame (lib/viewCache.ts).
  // svelte-ignore state_referenced_locally
  const source = mixSource(id);
  const cached = peek(source);
  let mix = $state<MixDetail | null>(cached ?? null);
  let loading = $state(cached === undefined);
  let error = $state("");

  const view = createTrackView(() => mix?.tracks ?? []);
  const artwork = $derived(mix?.artwork ?? mix?.tracks[0]?.album?.cover ?? null);

  // "50 TRACKS (3:12:44)" — mixes carry no metadata totals, so sum the tracks.
  const stats = $derived.by(() => {
    const tracks = mix?.tracks ?? [];
    const count = tracks.length;
    const seconds = tracks.reduce((sum, t) => sum + t.duration, 0);
    const label = `${count} ${count === 1 ? "TRACK" : "TRACKS"}`;
    return seconds > 0 ? `${label} (${formatTotalDuration(seconds)})` : label;
  });

  async function load() {
    if (peek(source) === undefined) loading = true;
    error = "";
    try {
      await loadCached(source, (loaded) => (mix = loaded));
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    untrack(load);
  });

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

  async function playFrom(index: number) {
    const prepared = await trackFlags.filterPlayable(view.items, index);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function share() {
    const copied = await copyText(shareUrl("mix", id));
    toasts.show(copied ? "Link copied" : "Couldn't copy link", {
      severity: copied ? "success" : "error",
    });
  }
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if mix}
  {@const data = mix}
  <div class="view-fill">
    <DetailHeader
      kicker="Mix"
      title={data.title}
      {stats}
      onPlay={playAll}
      onShuffle={shuffleAll}
      disabled={view.items.length === 0}
    >
      {#snippet cover()}
        <Cover uuid={artwork} size={640} radius={12} alt={`${data.title} cover`} />
      {/snippet}
      {#snippet actions()}
        <DetailAction
          icon="list-plus"
          label="Add"
          title="Add mix to playlist"
          onclick={() => playlistActions.openPicker(view.items)}
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
      onPlay={playFrom}
      playingTrackId={player.state.current?.track_id ?? null}
      playbackRunning={player.state.playing}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    />
  </div>
{:else}
  <EmptyState message="Mix not found." />
{/if}


