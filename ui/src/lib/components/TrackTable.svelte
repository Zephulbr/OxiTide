<script lang="ts">
  import type { Track } from "../api";
  import { api, trackToQueueItem } from "../api";
  import { copyText } from "../clipboard";
  import { formatDuration } from "../format";
  import { menuFor } from "../menu.svelte";
  import { trackMenuItems } from "../menus";
  import { windowRange } from "../virtual";
  import { appearance } from "../appearance.svelte";
  import type { Favorites } from "../favorites.svelte";
  import type { TrackView } from "../trackView.svelte";
  import { trackDrag, type DragParams } from "../drag";
  import Loading from "./Loading.svelte";
  import Icon from "./Icon.svelte";
  import Cover from "./Cover.svelte";
  let {
    view,
    favorites,
    onPlay,
    onPlayAll,
    onEndReached,
    onAddToPlaylist,
    playlistActions,
    maxHeight = "min(60vh, 560px)",
    showCovers = true,
    playingTrackId = null,
  }: {
    view: TrackView;
    favorites: Favorites;
    onPlay?: (index: number) => void | Promise<void>;
    onPlayAll?: () => void | Promise<void>;
    /// Called when the list is scrolled near its end; used for lazy paging.
    onEndReached?: () => void;
    /// Renders the "add to playlist" row action (list-plus icon, distinct
    /// from the plus-icon queue action).
    onAddToPlaylist?: (track: Track) => void;
    /// Playlist-view row editing (move up/down, remove). `disabled` is set by
    /// the parent until the pager has the complete list; `onDrop` (Task 8)
    /// receives absolute source-order indices from drag-and-drop.
    playlistActions?: {
      onRemove(index: number): void;
      onMove(index: number, delta: -1 | 1): void;
      busy: boolean;
      disabled: boolean;
      onDrop?(from: number, to: number): void;
    };
    maxHeight?: string;
    /// Cover thumbnails are redundant in single-album view (spec §7.3).
    showCovers?: boolean;
    /// Highlights the playing row (accent tint + title, ringed thumb).
    playingTrackId?: number | null;
  } = $props();

  /// Row height follows the density setting (spec §7.4): 38 / 28 px.
  const rowHeight = $derived(appearance.density === "compact" ? 28 : 38);
  const OVERSCAN = 8;
  const END_THRESHOLD = 400;

  let scrollTop = $state(0);
  let viewport = $state(0);
  let loadingAll = $state(false);

  const rows = $derived(view.items);
  const range = $derived(windowRange(rows.length, scrollTop, viewport, rowHeight, OVERSCAN));
  const slice = $derived(rows.slice(range.first, range.last));
  const columns = $derived(showCovers ? 5 : 4);

  let dragFrom = $state<number | null>(null);
  let dropAt = $state<number | null>(null);
  let theadHeight = $state(0);

  const canDrag = $derived(
    playlistActions !== undefined &&
      !playlistActions.busy &&
      !playlistActions.disabled &&
      !view.engaged,
  );

  const drag = $derived<DragParams>({
    enabled: () => canDrag,
    count: () => rows.length,
    rowHeight: () => rowHeight,
    offset: () => theadHeight,
    onChange: (from, at) => {
      dragFrom = from;
      dropAt = at;
    },
    onDrop: (from, to) => playlistActions?.onDrop?.(from, to),
  });

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

  function onRowDoubleClick(event: MouseEvent, index: number) {
    if (loadingAll) return;
    const target = event.target;
    if (target instanceof Element && target.closest("button, a, input, select, textarea")) return;
    void play(index);
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
    <button onclick={() => view.setFilter("")} aria-label="Clear filter"><Icon name="x" size={14} /></button>
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
  use:trackDrag={drag}
>
  <table>
    <thead bind:clientHeight={theadHeight}>
      <tr>
        {#if showCovers}<th class="thumb-head"></th>{/if}
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
        <tr aria-hidden="true"><td colspan={columns} style="height:{range.padTop}px;padding:0;border:0"></td></tr>
      {/if}
      {#each slice as t, i (`${t.id}-${range.first + i}`)}
        {@const index = range.first + i}
        <tr
          class="row"
          class:dragging={dragFrom === index}
          class:playing={playingTrackId !== null && t.id === playingTrackId}
          draggable={canDrag}
          data-drag-index={canDrag ? index : undefined}
          ondblclick={(event) => onRowDoubleClick(event, index)}
          use:menuFor={() =>
            trackMenuItems({
              track: t,
              index,
              trackCount: rows.length,
              favorites,
              play: () => play(index),
              addToQueue: () => api.queueAdd([trackToQueueItem(t)]),
              addToPlaylist: () => onAddToPlaylist?.(t),
              copyTitle: () => void copyText(t.title),
              playlist: playlistActions && { ...playlistActions, canMove: !view.engaged },
            })}
        >
          <td>
            {#if canDrag}
              <span class="handle" title="Drag to reorder" aria-hidden="true"><Icon name="grip-vertical" size={14} /></span>
            {/if}
            <button onclick={() => play(index)} disabled={loadingAll} aria-label="Play"><Icon name="play" size={14} /></button>
            <button
              onclick={() => api.queueAdd([trackToQueueItem(t)])}
              aria-label="Add to queue"
              title="Add to queue"
            ><Icon name="plus" size={14} /></button>
            {#if onAddToPlaylist}
              <button
                onclick={() => onAddToPlaylist(t)}
                aria-label="Add to playlist"
                title="Add to playlist"
              ><Icon name="list-plus" size={14} /></button>
            {/if}
            <button
              class:on={favorites.hasTrack(t.id)}
              onclick={() => favorites.toggleTrack(t.id)}
              aria-label={favorites.hasTrack(t.id) ? "Unfavorite" : "Favorite"}
              title={favorites.hasTrack(t.id) ? "Unfavorite" : "Favorite"}
            ><Icon name="heart" size={14} /></button>
            {#if playlistActions}
              {#if !view.engaged}
                <button
                  onclick={() => playlistActions.onMove(index, -1)}
                  disabled={playlistActions.busy || playlistActions.disabled || index === 0}
                  aria-label="Move up"
                  title="Move up"
                ><Icon name="chevron-up" size={14} /></button>
                <button
                  onclick={() => playlistActions.onMove(index, 1)}
                  disabled={playlistActions.busy || playlistActions.disabled || index === rows.length - 1}
                  aria-label="Move down"
                  title="Move down"
                ><Icon name="chevron-down" size={14} /></button>
              {/if}
              <button
                onclick={() => playlistActions.onRemove(index)}
                disabled={playlistActions.busy || playlistActions.disabled}
                aria-label="Remove from playlist"
                title="Remove from playlist"
              ><Icon name="x" size={14} /></button>
            {/if}
          </td>
          {#if showCovers}
            <td class="thumb-cell">
              <span class="thumb">
                <Cover uuid={t.album?.cover} size={80} radius={6} alt="" />
                <span class="thumb-play" aria-hidden="true"><Icon name="play" size={14} /></span>
              </span>
            </td>
          {/if}
          <td class="muted">{index + 1}</td>
          <td class="title-cell"><div class="title-text">{t.title}<span class="muted"> · {t.artists.map((a) => a.name).join(", ")}</span></div></td>
          <td style="text-align:right" class="muted">{formatDuration(t.duration)}</td>
        </tr>
      {/each}
      {#if range.padBottom > 0}
        <tr aria-hidden="true"><td colspan={columns} style="height:{range.padBottom}px;padding:0;border:0"></td></tr>
      {/if}
      {#if rows.length === 0}
        <tr><td colspan={columns} class="muted">{view.filter.trim() ? `No tracks match “${view.filter.trim()}”` : "No tracks"}</td></tr>
      {/if}
    </tbody>
  </table>
  {#if dropAt !== null}
    <div class="drop-line" style="top:{theadHeight + dropAt * rowHeight}px"></div>
  {/if}
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
    position: relative;
  }
  .handle {
    cursor: grab;
    color: var(--muted);
    padding-right: 4px;
  }
  .row.dragging {
    opacity: 0.5;
  }
  .drop-line {
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--accent);
    pointer-events: none;
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
    height: var(--row-height);
    border-bottom: 1px solid var(--line);
    user-select: none;
  }
  .thumb-head {
    width: var(--thumb-size);
    padding: 0;
  }
  .thumb-cell {
    width: var(--thumb-size);
    padding: 4px 8px 4px 0;
  }
  .thumb {
    position: relative;
    display: block;
    width: var(--thumb-size);
  }
  .thumb-play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.45);
    border-radius: 6px;
    opacity: 0;
    transition: opacity var(--dur-micro);
    color: #fff;
  }
  .row:hover .thumb-play {
    opacity: 1;
  }
  .row.playing {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .row.playing .title-text {
    color: var(--accent);
  }
  .row.playing .thumb :global(img) {
    box-shadow: 0 0 0 1.5px var(--accent);
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
  .on :global(svg) {
    fill: currentColor;
  }
  td button {
    display: inline-grid;
    place-items: center;
    vertical-align: middle;
  }
</style>
