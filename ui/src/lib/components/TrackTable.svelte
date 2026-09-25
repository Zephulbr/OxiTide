<script lang="ts">
  import type { Track } from "../api";
  import { api, trackToQueueItem } from "../api";
  import { formatDuration } from "../format";
  import { menuFor } from "../menu.svelte";
  import { trackMenuItems } from "../menus";
  import type { Favorites } from "../favorites.svelte";
  import type { Player } from "../player.svelte";
  let {
    tracks,
    player,
    favorites,
    onPlay,
    onPlayAll,
    onEndReached,
    onAddToPlaylist,
    playlistActions,
    maxHeight = "min(60vh, 560px)",
  }: {
    tracks: Track[];
    player: Player;
    favorites: Favorites;
    onPlay?: (index: number) => void | Promise<void>;
    onPlayAll?: () => void | Promise<void>;
    /// Called when the list is scrolled near its end; used for lazy paging.
    onEndReached?: () => void;
    /// Renders the "add to playlist" row action. Uses ⊕ to stay distinct
    /// from the "＋" queue action.
    onAddToPlaylist?: (track: Track) => void;
    /// Playlist-view row editing (move up/down, remove). `disabled` is set by
    /// the parent until the pager has the complete list.
    playlistActions?: {
      onRemove(index: number): void;
      onMove(index: number, delta: -1 | 1): void;
      busy: boolean;
      disabled: boolean;
    };
    maxHeight?: string;
  } = $props();

  /// Row height in px; keep in sync with the `.row` style below.
  const ROW_HEIGHT = 36;
  const OVERSCAN = 8;
  const END_THRESHOLD = 400;

  let scrollTop = $state(0);
  let viewport = $state(0);
  let loadingAll = $state(false);

  const visible = $derived(Math.max(1, Math.ceil(viewport / ROW_HEIGHT) + OVERSCAN * 2));
  // Clamp to the list so a refreshed/shrunk list cannot leave the window
  // pointing past the end (which would render a blank table).
  const first = $derived(
    Math.min(
      Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN),
      Math.max(0, tracks.length - visible),
    ),
  );
  const last = $derived(Math.min(tracks.length, first + visible));
  const slice = $derived(tracks.slice(first, last));
  const padTop = $derived(first * ROW_HEIGHT);
  const padBottom = $derived(Math.max(0, (tracks.length - last) * ROW_HEIGHT));

  async function play(index: number) {
    if (onPlay) {
      loadingAll = true;
      try {
        await onPlay(index);
      } finally {
        loadingAll = false;
      }
      return;
    }
    api.playTracks(tracks.map(trackToQueueItem), index);
  }

  async function playAll() {
    if (onPlayAll) {
      loadingAll = true;
      try {
        await onPlayAll();
      } finally {
        loadingAll = false;
      }
      return;
    }
    play(0);
  }

  function onScroll(event: Event) {
    const element = event.currentTarget as HTMLElement;
    scrollTop = element.scrollTop;
    if (element.scrollHeight - element.scrollTop - element.clientHeight < END_THRESHOLD) {
      onEndReached?.();
    }
  }
</script>

<div
  class="table-scroll"
  style="max-height:{maxHeight}"
  bind:clientHeight={viewport}
  onscroll={onScroll}
>
  <table>
    <thead>
      <tr>
        <th style="text-align:left"><button onclick={playAll} disabled={loadingAll}>{loadingAll ? "Loading all…" : "Play all"}</button></th>
        <th style="text-align:left">#</th>
        <th style="text-align:left">Title</th>
        <th style="text-align:right">Time</th>
      </tr>
    </thead>
    <tbody>
      {#if padTop > 0}
        <tr aria-hidden="true"><td colspan="4" style="height:{padTop}px;padding:0;border:0"></td></tr>
      {/if}
      {#each slice as t, i (`${t.id}-${first + i}`)}
        {@const index = first + i}
        <tr
          class="row"
          use:menuFor={() =>
            trackMenuItems({
              track: t,
              index,
              trackCount: tracks.length,
              favorites,
              play: () => play(index),
              addToQueue: () => api.queueAdd([trackToQueueItem(t)]),
              addToPlaylist: () => onAddToPlaylist?.(t),
              playlist: playlistActions,
            })}
        >
          <td>
            <button onclick={() => play(index)} disabled={loadingAll} aria-label="Play">▶</button>
            <button
              onclick={() => api.queueAdd([trackToQueueItem(t)])}
              aria-label="Add to queue"
              title="Add to queue"
            >＋</button>
            {#if onAddToPlaylist}
              <button
                onclick={() => onAddToPlaylist(t)}
                aria-label="Add to playlist"
                title="Add to playlist"
              >⊕</button>
            {/if}
            <button
              class:on={favorites.hasTrack(t.id)}
              onclick={() => favorites.toggleTrack(t.id)}
              aria-label={favorites.hasTrack(t.id) ? "Unfavorite" : "Favorite"}
              title={favorites.hasTrack(t.id) ? "Unfavorite" : "Favorite"}
            >{favorites.hasTrack(t.id) ? "♥" : "♡"}</button>
            {#if playlistActions}
              <button
                onclick={() => playlistActions.onMove(index, -1)}
                disabled={playlistActions.busy || playlistActions.disabled || index === 0}
                aria-label="Move up"
                title="Move up"
              >↑</button>
              <button
                onclick={() => playlistActions.onMove(index, 1)}
                disabled={playlistActions.busy || playlistActions.disabled || index === tracks.length - 1}
                aria-label="Move down"
                title="Move down"
              >↓</button>
              <button
                onclick={() => playlistActions.onRemove(index)}
                disabled={playlistActions.busy || playlistActions.disabled}
                aria-label="Remove from playlist"
                title="Remove from playlist"
              >✕</button>
            {/if}
          </td>
          <td class="muted">{index + 1}</td>
          <td class="title-cell"><div class="title-text">{t.title}<span class="muted"> · {t.artists.map((a) => a.name).join(", ")}</span></div></td>
          <td style="text-align:right" class="muted">{formatDuration(t.duration)}</td>
        </tr>
      {/each}
      {#if padBottom > 0}
        <tr aria-hidden="true"><td colspan="4" style="height:{padBottom}px;padding:0;border:0"></td></tr>
      {/if}
      {#if tracks.length === 0}
        <tr><td colspan="4" class="muted">No tracks</td></tr>
      {/if}
    </tbody>
  </table>
</div>

<style>
  .table-scroll {
    overflow: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  thead th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
  }
  .row {
    height: 36px;
    border-bottom: 1px solid var(--line);
  }
  .title-cell {
    max-width: 0;
    width: 100%;
  }
  .title-text {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .on {
    color: var(--accent);
  }
</style>
