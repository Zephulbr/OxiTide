<script lang="ts">
  import type { Track } from "../api";
  import { api, trackToQueueItem } from "../api";
  import { formatDuration } from "../format";
  import { menuFor } from "../menu.svelte";
  import { trackMenuItems } from "../menus";
  import { windowRange } from "../virtual";
  import type { Favorites } from "../favorites.svelte";
  import type { TrackView } from "../trackView.svelte";
  import Loading from "./Loading.svelte";
  let {
    view,
    favorites,
    onPlay,
    onPlayAll,
    onEndReached,
    onAddToPlaylist,
    playlistActions,
    maxHeight = "min(60vh, 560px)",
  }: {
    view: TrackView;
    favorites: Favorites;
    onPlay?: (index: number) => void | Promise<void>;
    onPlayAll?: () => void | Promise<void>;
    /// Called when the list is scrolled near its end; used for lazy paging.
    onEndReached?: () => void;
    /// Renders the "add to playlist" row action. Uses ⊕ to stay distinct
    /// from the "＋" queue action.
    onAddToPlaylist?: (track: Track) => void;
    /// Playlist-view row editing (move up/down, remove). `disabled` is set by
    /// the parent until the pager has the complete list; `onDrop` (Task 8)
    /// receives absolute source-order indices from drag-and-drop.
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

  const rows = $derived(view.items);
  const range = $derived(windowRange(rows.length, scrollTop, viewport, ROW_HEIGHT, OVERSCAN));
  const slice = $derived(rows.slice(range.first, range.last));

  function indicator(key: "title" | "duration"): string {
    if (view.sort !== key) return "";
    return view.direction === "asc" ? "▲" : "▼";
  }

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
    api.playTracks(rows.map(trackToQueueItem), index);
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

<div class="table-tools">
  <input
    class="filter"
    placeholder="Filter tracks"
    value={view.filter}
    oninput={(event) => view.setFilter(event.currentTarget.value)}
  />
  {#if view.filter.trim() !== ""}
    <button onclick={() => view.setFilter("")} aria-label="Clear filter">✕</button>
  {/if}
  {#if view.preparing}
    <Loading label="Loading all tracks…" />
  {/if}
  {#if view.error}
    <span class="muted">{view.error}</span>
    <button onclick={() => view.retry()}>Retry</button>
  {/if}
</div>

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
        <th style="text-align:left">
          <button class="sort" title="Original order" onclick={() => view.clearSort()}>#</button>
        </th>
        <th style="text-align:left">
          <button class="sort" disabled={view.preparing} onclick={() => view.toggleSort("title")}>
            Title {indicator("title")}
          </button>
        </th>
        <th style="text-align:right">
          <button class="sort" disabled={view.preparing} onclick={() => view.toggleSort("duration")}>
            Time {indicator("duration")}
          </button>
        </th>
      </tr>
    </thead>
    <tbody>
      {#if range.padTop > 0}
        <tr aria-hidden="true"><td colspan="4" style="height:{range.padTop}px;padding:0;border:0"></td></tr>
      {/if}
      {#each slice as t, i (`${t.id}-${range.first + i}`)}
        {@const index = range.first + i}
        <tr
          class="row"
          use:menuFor={() =>
            trackMenuItems({
              track: t,
              index,
              trackCount: rows.length,
              favorites,
              play: () => play(index),
              addToQueue: () => api.queueAdd([trackToQueueItem(t)]),
              addToPlaylist: () => onAddToPlaylist?.(t),
              playlist: playlistActions && { ...playlistActions, canMove: !view.engaged },
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
              {#if !view.engaged}
                <button
                  onclick={() => playlistActions.onMove(index, -1)}
                  disabled={playlistActions.busy || playlistActions.disabled || index === 0}
                  aria-label="Move up"
                  title="Move up"
                >↑</button>
                <button
                  onclick={() => playlistActions.onMove(index, 1)}
                  disabled={playlistActions.busy || playlistActions.disabled || index === rows.length - 1}
                  aria-label="Move down"
                  title="Move down"
                >↓</button>
              {/if}
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
      {#if range.padBottom > 0}
        <tr aria-hidden="true"><td colspan="4" style="height:{range.padBottom}px;padding:0;border:0"></td></tr>
      {/if}
      {#if rows.length === 0}
        <tr><td colspan="4" class="muted">{view.filter.trim() ? `No tracks match “${view.filter.trim()}”` : "No tracks"}</td></tr>
      {/if}
    </tbody>
  </table>
</div>

<style>
  .table-tools {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .table-tools .filter {
    flex: 0 1 260px;
  }
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
  .sort {
    background: transparent;
    border: 0;
    color: inherit;
    padding: 0;
    cursor: pointer;
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
