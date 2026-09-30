<script lang="ts">
  import type { Track } from "../api";
  import { api, trackToQueueItem } from "../api";
  import { copyText } from "../clipboard";
  import { formatDuration } from "../format";
  import { menuFor } from "../menu.svelte";
  import { trackMenuItems } from "../menus";
  import { trackQualityLabel } from "../quality";
  import { playTrackRadio } from "../radio";
  import type { Toasts } from "../toasts.svelte";
  import { windowRange } from "../virtual";
  import { appearance } from "../appearance.svelte";
  import { rowHeightFor } from "../rowHeight";
  import type { Favorites } from "../favorites.svelte";
  import type { TrackView } from "../trackView.svelte";
  import { trackDrag, type DragParams } from "../drag";
  import Loading from "./Loading.svelte";
  import Icon from "./Icon.svelte";
  import Cover from "./Cover.svelte";
  let {
    view,
    favorites,
    toasts,
    onPlay,
    onEndReached,
    onAddToPlaylist,
    playlistActions,
    maxHeight = "min(60vh, 560px)",
    fill = false,
    showAlbum = true,
    playingTrackId = null,
  }: {
    view: TrackView;
    favorites: Favorites;
    toasts: Toasts;
    onPlay?: (index: number) => void | Promise<void>;
    /// Called when the list is scrolled near its end; used for lazy paging.
    onEndReached?: () => void;
    /// Renders the "add to playlist" row action (list-plus icon, distinct
    /// from the plus-icon queue action).
    onAddToPlaylist?: (track: Track) => void;
    /// Playlist-view row editing via drag-and-drop and the context menu.
    /// `disabled` is set by the parent until the pager has the complete list;
    /// `onDrop` receives absolute source-order indices from drag-and-drop.
    playlistActions?: {
      onRemove(index: number): void;
      onMove(index: number, delta: -1 | 1): void;
      busy: boolean;
      disabled: boolean;
      onDrop?(from: number, to: number): void;
    };
    maxHeight?: string;
    /// Grow to fill the parent's leftover height (parent is a .view-fill flex
    /// column) instead of capping at maxHeight.
    fill?: boolean;
    /// The album-name column is redundant in single-album view (spec §7.3).
    showAlbum?: boolean;
    /// Highlights the playing row (accent tint + title, ringed thumb).
    playingTrackId?: number | null;
  } = $props();

  /// Row height follows the density setting (spec §7.4): 46 / 38 px,
  /// mirrored from --row-height via rowHeightFor (guarded by rowHeight.test.ts).
  const rowHeight = $derived(rowHeightFor(appearance.density));
  const OVERSCAN = 8;
  const END_THRESHOLD = 400;

  let scrollTop = $state(0);
  const scrolled = $derived(scrollTop > 0);
  let viewport = $state(0);
  let loadingAll = $state(false);

  const rows = $derived(view.items);
  const range = $derived(windowRange(rows.length, scrollTop, viewport, rowHeight, OVERSCAN));
  const slice = $derived(rows.slice(range.first, range.last));
  const columns = $derived(showAlbum ? 8 : 7);

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
    data-perf-filter
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
  class:fill
  class:scrolled
  style:max-height={fill ? "none" : maxHeight}
  bind:clientHeight={viewport}
  onscroll={onScroll}
  data-perf-scroll
  use:trackDrag={drag}
>
  <div
    class="head-frost"
    aria-hidden="true"
    style="height:{theadHeight}px;margin-bottom:-{theadHeight}px"
  ></div>
  <table>
    <thead bind:clientHeight={theadHeight}>
      <tr>
        <th style="text-align:right">
          <button class="sort" title="Original order" onclick={() => view.clearSort()}>#</button>
        </th>
        <th class="thumb-head"></th>
        <th style="text-align:left">
          <button class="sort" disabled={view.preparing} onclick={() => view.toggleSort("title")}>
            Title {indicator("title")}
          </button>
        </th>
        <th style="text-align:left">Artist</th>
        {#if showAlbum}<th style="text-align:left">Album</th>{/if}
        <th style="text-align:right">
          <button class="sort" disabled={view.preparing} onclick={() => view.toggleSort("duration")}>
            Time {indicator("duration")}
          </button>
        </th>
        <th style="text-align:left">Quality</th>
        <th class="actions-head"></th>
      </tr>
    </thead>
    <tbody>
      {#if range.padTop > 0}
        <tr aria-hidden="true"><td colspan={columns} style="height:{range.padTop}px;padding:0;border:0"></td></tr>
      {/if}
      {#each slice as t, i (`${t.id}-${range.first + i}`)}
        {@const index = range.first + i}
        {@const quality = trackQualityLabel(t.audioQuality)}
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
              startRadio: () => void playTrackRadio(t.id, toasts),
              addToQueue: () => api.queueAdd([trackToQueueItem(t)]),
              addToPlaylist: () => onAddToPlaylist?.(t),
              copyTitle: () => void copyText(t.title),
              playlist: playlistActions && { ...playlistActions, canMove: !view.engaged },
            })}
        >
          <td class="muted tnum num-cell">{index + 1}</td>
          <td class="thumb-cell">
            <button class="thumb" onclick={() => play(index)} disabled={loadingAll} aria-label="Play" title="Play">
              <Cover uuid={t.album?.cover} size={80} radius={0} alt="" />
              <span class="thumb-play" aria-hidden="true"><Icon name="play" size={14} /></span>
            </button>
          </td>
          <td class="title-cell"><div class="title-text">{t.title}</div></td>
          <td class="text-cell muted"><div class="cell-text">{t.artists.map((a) => a.name).join(", ")}</div></td>
          {#if showAlbum}
            <td class="text-cell muted"><div class="cell-text">{t.album?.title ?? ""}</div></td>
          {/if}
          <td class="muted tnum nowrap-cell time-cell">{formatDuration(t.duration)}</td>
          <td class="nowrap-cell">
            {#if quality}<span class="badge" class:max={quality === "MAX"}>{quality}</span>{/if}
          </td>
          <td class="actions-cell">
            <button
              class="reveal"
              onclick={() => api.queueAdd([trackToQueueItem(t)])}
              aria-label="Add to queue"
              title="Add to queue"
            ><Icon name="plus" size={14} /></button>
            {#if onAddToPlaylist}
              <button
                class="reveal"
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
          </td>
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
  /* fill mode: own the .view-fill parent's leftover height and scroll
     internally; keep a floor so a tall header can't collapse it to zero. */
  .table-scroll.fill {
    flex: 1 1 auto;
    min-height: 140px;
    /* Rows scroll behind the floating player bar; the clearance lets the last
       row rise clear of it. */
    padding-bottom: calc(var(--player-height) + 8px);
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
  /* One frost layer for the sticky header. No per-cell backdrop-filter and no
     mask: both are unreliable in Chromium inside a scroller (masked blur can
     render as solid dark blocks). At rest the header is untinted so it matches
     the surface above it exactly; the frost fades in only once rows scroll
     beneath — atmosphere's blur carries it, instrument's veil is opaque. The
     positive z-index is load-bearing: row thumbnail buttons are positioned
     and would otherwise paint above this layer, escaping the backdrop blur. */
  .head-frost {
    position: sticky;
    top: 0;
    z-index: 1;
    pointer-events: none;
    backdrop-filter: blur(var(--glass-blur));
    transition: box-shadow var(--dur-state) var(--ease-in);
  }
  .table-scroll.scrolled .head-frost {
    box-shadow: 0 14px 18px -14px rgba(0, 0, 0, 0.55);
  }
  .head-frost::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    bottom: -12px;
    height: 12px;
    background: linear-gradient(to bottom, var(--glass-veil), transparent);
    opacity: 0;
    transition: opacity var(--dur-state) var(--ease-in);
  }
  .table-scroll.scrolled .head-frost::after {
    opacity: 1;
  }
  thead th {
    position: sticky;
    top: 0;
    z-index: 2;
    background: transparent;
    transition: background-color var(--dur-state) var(--ease-in);
    padding: 4px 12px 4px 0;
  }
  .table-scroll.scrolled thead th {
    background: var(--glass-veil);
  }
  thead th.thumb-head {
    padding: 0;
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
  .row:hover {
    background: var(--surface-2);
  }
  td {
    padding: 0 12px 0 0;
  }
  /* The cover fills the row top to bottom; -1px keeps it inside the row's
     content box so the virtualization row math (var(--row-height)) holds. */
  .thumb-cell {
    padding: 0 12px 0 0;
  }
  .thumb {
    position: relative;
    display: block;
    padding: 0;
    width: calc(var(--row-height) - 1px);
    height: calc(var(--row-height) - 1px);
  }
  .thumb-play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.45);
    opacity: 0;
    transition: opacity var(--dur-micro);
    color: var(--text);
  }
  .row:hover .thumb-play {
    opacity: 1;
  }
  .row.playing {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  .row.playing:hover {
    background: color-mix(in srgb, var(--accent) 24%, transparent);
  }
  .row.playing .title-text {
    color: var(--accent);
  }
  /* Text columns share the leftover width proportionally (title widest) so
     the row breathes at any window width; the inner divs ellipsis. The
     remaining columns size to their (nowrap) content, actions last. */
  .title-cell {
    width: 34%;
  }
  .text-cell {
    width: 22%;
  }
  .num-cell,
  .time-cell {
    text-align: right;
  }
  .title-text,
  .cell-text {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .nowrap-cell {
    white-space: nowrap;
  }
  .actions-cell {
    padding: 0;
    white-space: nowrap;
    text-align: right;
  }
  .actions-cell button + button {
    margin-left: 6px;
  }
  /* Queue/playlist actions hide until the row is hovered or focused (heart
     stays visible); pointer-events follow so hidden buttons can't be hit. */
  .actions-cell .reveal {
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur-micro);
  }
  .row:hover .actions-cell .reveal,
  .row:focus-within .actions-cell .reveal {
    opacity: 1;
    pointer-events: auto;
  }
  /* Same tag look as the PlayerBar/NowPlaying quality badge. */
  .badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 8px;
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .badge.max {
    border-color: var(--dyn-tertiary);
    color: var(--dyn-tertiary);
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
  td button.thumb {
    display: block;
  }
</style>
