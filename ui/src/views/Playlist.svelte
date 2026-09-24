<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Playlist as PlaylistT } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import type { Player } from "../lib/player.svelte";
  let { id, player }: { id: string; player: Player } = $props();

  let playlist = $state<PlaylistT | null>(null);
  let loading = $state(true);
  let error = $state("");

  // `$derived` so navigating to another playlist rebuilds the pager; the new
  // key hydrates from the session cache when that playlist was seen before.
  // `dedupe: false`: a playlist may legitimately repeat a track, and the queue
  // must match the playlist exactly. `prefetchWindow: 100` is the window size
  // the playlist endpoint is known to honor.
  const pager = $derived(
    createTrackPager(
      `playlist:${id}`,
      (offset, limit) => api.getPlaylistTracksPage(id, offset, limit),
      { dedupe: false, prefetchWindow: 100 },
    ),
  );

  async function load() {
    loading = true;
    error = "";
    try {
      playlist = await api.getPlaylist(id);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });

  // The first page paints fast, then the pager prefetches the remainder in the
  // background. The effect tracks `pager` so navigating to another playlist
  // re-runs it; `untrack` keeps the call itself off the dependency graph.
  $effect(() => {
    const current = pager;
    untrack(() => current.loadMore());
  });

  async function playFrom(index: number) {
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(pager.items.map(trackToQueueItem), index);
  }

  async function playAll() {
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(pager.items.map(trackToQueueItem), 0);
  }
</script>

{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else}
  {#if playlist}
    <h2>{playlist.title}</h2>
  {/if}
  <p>
    <button onclick={() => pager.refresh()}>Refresh</button>
    <span class="muted">
      {#if pager.unavailable > 0}
        {pager.items.length} tracks ({pager.unavailable} unavailable)
      {:else}
        {pager.items.length} tracks
      {/if}
    </span>
  </p>
  <TrackTable
    tracks={pager.items}
    {player}
    onPlay={playFrom}
    onPlayAll={playAll}
    onEndReached={() => void pager.loadMore()}
  />
  {#if pager.error}
    <ErrorState message={pager.error} onRetry={() => pager.loadMore()} />
  {/if}
  {#if pager.loading}<p class="muted">Loading…</p>{/if}
{/if}
