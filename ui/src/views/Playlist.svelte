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
  let sentinel = $state<HTMLElement | null>(null);

  // `$derived` so navigating to another playlist rebuilds the pager; the new
  // key hydrates from the session cache when that playlist was seen before.
  // `dedupe = false`: a playlist may legitimately repeat a track, and the
  // queue must match the playlist exactly.
  const pager = $derived(
    createTrackPager(
      `playlist:${id}`,
      (offset, limit) => api.getPlaylistTracksPage(id, offset, limit),
      100,
      false,
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

  // Eager first page: the sentinel alone would leave the list empty when it
  // starts below the fold. `untrack` keeps this a one-shot mount action.
  $effect(() => {
    untrack(() => pager.loadMore());
  });

  $effect(() => {
    if (!sentinel || pager.done) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) pager.loadMore();
    });
    observer.observe(sentinel);
    return () => observer.disconnect();
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
    <p class="muted">{playlist.numberOfTracks} tracks</p>
  {/if}
  <p>
    <button onclick={() => pager.refresh()}>Refresh</button>
    <span class="muted">{pager.items.length} of {pager.total}</span>
  </p>
  <TrackTable tracks={pager.items} {player} onPlay={playFrom} onPlayAll={playAll} />
  {#if pager.error}
    <ErrorState message={pager.error} onRetry={() => pager.loadMore()} />
  {/if}
  {#if pager.loading}<p class="muted">Loading…</p>{/if}
  {#if !pager.done && !pager.error}
    <div bind:this={sentinel} style="height:1px"></div>
  {/if}
{/if}
