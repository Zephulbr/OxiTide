<script lang="ts">
  import { tick } from "svelte";
  import type { Track } from "../api";
  import {
    volumeDisplayRows,
    type VolumeSection,
    type VolumeDisplayRow,
  } from "../albumVolumes";
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
  import { trackFlags } from "../trackFlags.svelte";
  import { trackInfo } from "../trackInfo.svelte";
  import { rowHeightFor } from "../rowHeight";
  import type { Favorites } from "../favorites.svelte";
  import type { TrackView } from "../trackView.svelte";
  import { trackDrag, type DragParams } from "../drag";
  import Icon from "./Icon.svelte";
  import Cover from "./Cover.svelte";
  import TrackFilter from "./TrackFilter.svelte";
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
    showFilter = true,
    /// Multi-volume album groups (album page); ignored while a sort/filter is
    /// engaged. Grouped lists render whole (no virtualization) and section
    /// headings reset the row number per volume.
    sections,
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
    /// Render the filter/status toolbar above the table. Views that surface
    /// the filter in their header (playlist) pass false.
    showFilter?: boolean;
    sections?: VolumeSection[];
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
  const grouping = $derived(sections !== undefined && !view.engaged ? sections : null);
  const range = $derived(
    grouping
      ? { first: 0, last: rows.length, padTop: 0, padBottom: 0 }
      : windowRange(rows.length, scrollTop, viewport, rowHeight, OVERSCAN),
  );
  const slice = $derived(grouping ? rows : rows.slice(range.first, range.last));

  /// Grouped rows resolve their index through identity against `rows`
  /// (source order), which can differ from display order when a payload
  /// interleaves volumes.
  const rowSourceIndex = $derived.by(() => {
    const index = new Map<Track, number>();
    rows.forEach((track, i) => {
      if (!index.has(track)) index.set(track, i);
    });
    return index;
  });

  /// Flat rows when ungrouped; heading rows interleaved with disc-scoped
  /// numbers when grouped.
  const displayRows = $derived.by<VolumeDisplayRow[]>(() => {
    if (!grouping) {
      return slice.map((track, i) => ({
        key: `${track.id}-${range.first + i}`,
        label: null,
        track,
        index: range.first + i,
        number: range.first + i + 1,
      }));
    }
    return volumeDisplayRows(grouping, rowSourceIndex);
  });
  const columns = $derived(showAlbum ? 8 : 7);

  // Rows render immediately; flags land asynchronously when the AI filter or
  // content labels are on (the store's `ensure` is a no-op when both are off).
  $effect(() => {
    trackFlags.recordTracks(slice);
    void trackFlags.ensure(slice.map((t) => t.id));
  });

  let dragFrom = $state<number | null>(null);
  let dropAt = $state<number | null>(null);
  let theadHeight = $state(0);

  const canDrag = $derived(
    sections === undefined &&
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

  function blockedNotice(title: string) {
    toasts.show(
      `"${title}" is hidden by your content filters. Change them in Settings → Content.`,
      { severity: "info" },
    );
  }

  function queueOne(t: Track) {
    if (trackFlags.isBlocked(t)) {
      blockedNotice(t.title);
      return;
    }
    void api.queueAdd([trackToQueueItem(t)]);
  }

  async function play(index: number) {
    const target = rows[index];
    if (target && trackFlags.isBlocked(target)) {
      blockedNotice(target.title);
      return;
    }
    if (onPlay) {
      loadingAll = true;
      try {
        await onPlay(index);
      } finally {
        loadingAll = false;
      }
      return;
    }
    const prepared = await trackFlags.filterPlayable(rows, index);
    if (!prepared) {
      blockedNotice(target?.title ?? "This track");
      return;
    }
    await api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
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

  let focusedIndex = $state<number | null>(null);
  let scrollEl = $state<HTMLDivElement | null>(null);

  /// Single tab stop: the focused row, else the first row in the viewport.
  /// Both are clamped so a shrinking/filtered list never points past its end.
  const tabbableIndex = $derived(
    Math.max(
      0,
      Math.min(
        focusedIndex ?? Math.floor(scrollTop / Math.max(rowHeight, 1)),
        rows.length - 1,
      ),
    ),
  );

  /// Move the roving focus, scrolling the target into the rendered window
  /// first so a Page jump can land on a row that was not mounted yet.
  async function focusRow(index: number) {
    const target = Math.min(Math.max(0, index), Math.max(0, rows.length - 1));
    focusedIndex = target;
    const scroll = scrollEl;
    if (scroll && (target < range.first || target >= range.last)) {
      scroll.scrollTop = Math.max(0, target * rowHeight - scroll.clientHeight / 2);
      scrollTop = scroll.scrollTop;
    }
    await tick();
    scroll?.querySelector<HTMLElement>(`tr[data-row-index="${target}"]`)?.focus();
  }

  function onRowKeydown(event: KeyboardEvent, index: number) {
    // Keys on the row's own buttons keep their native behavior.
    if (event.target !== event.currentTarget) return;
    const page = Math.max(1, Math.floor(viewport / rowHeight) - 1);
    let target: number | null = null;
    switch (event.key) {
      case "ArrowDown":
        target = index + 1;
        break;
      case "ArrowUp":
        target = index - 1;
        break;
      case "Home":
        target = 0;
        break;
      case "End":
        target = rows.length - 1;
        break;
      case "PageDown":
        target = index + page;
        break;
      case "PageUp":
        target = index - page;
        break;
    }
    if (target !== null) {
      event.preventDefault();
      event.stopPropagation();
      void focusRow(target);
      return;
    }
    if ((event.key === "Enter" || event.key === " ") && !loadingAll) {
      event.preventDefault();
      event.stopPropagation();
      void play(index);
    }
  }
</script>

{#if showFilter}
  <div class="table-tools"><TrackFilter {view} /></div>
{/if}

<div
  class="table-scroll"
  class:fill
  class:scrolled
  bind:this={scrollEl}
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
  <table aria-label="Tracks" aria-rowcount={grouping ? undefined : rows.length + 1}>
    <thead bind:clientHeight={theadHeight}>
      <tr>
        <th scope="col" style="text-align:right">
          <button class="sort" title="Original order" onclick={() => view.clearSort()}>#</button>
        </th>
        <th scope="col" class="thumb-head"></th>
        <th
          scope="col"
          style="text-align:left"
          aria-sort={view.sort === "title" ? (view.direction === "asc" ? "ascending" : "descending") : undefined}
        >
          <button class="sort" disabled={view.preparing} onclick={() => view.toggleSort("title")}>
            Title <span aria-hidden="true">{indicator("title")}</span>
          </button>
        </th>
        <th scope="col" style="text-align:left">Artist</th>
        {#if showAlbum}<th scope="col" style="text-align:left">Album</th>{/if}
        <th
          scope="col"
          style="text-align:right"
          aria-sort={view.sort === "duration" ? (view.direction === "asc" ? "ascending" : "descending") : undefined}
        >
          <button class="sort" disabled={view.preparing} onclick={() => view.toggleSort("duration")}>
            Time <span aria-hidden="true">{indicator("duration")}</span>
          </button>
        </th>
        <th scope="col" style="text-align:left">Quality</th>
        <th scope="col" class="actions-head"></th>
      </tr>
    </thead>
    <tbody>
      {#if range.padTop > 0}
        <tr aria-hidden="true"><td colspan={columns} style="height:{range.padTop}px;padding:0;border:0"></td></tr>
      {/if}
      {#each displayRows as row (row.key)}
        {#if row.label !== null}
          <tr class="section-row">
            <th scope="colgroup" colspan={columns}>{row.label}</th>
          </tr>
        {:else if row.track !== undefined}
          {@const t = row.track}
          {@const index = row.index ?? 0}
          {@const quality = trackQualityLabel(t.audioQuality)}
          {@const reasons = trackFlags.blockReasons(t)}
          {@const tags = trackFlags.showLabels
            ? [...new Set([...reasons, ...trackFlags.labels(t)])]
            : reasons}
          <tr
            class="row"
            class:blocked={reasons.length > 0}
            class:dragging={dragFrom === index}
            class:playing={playingTrackId !== null && t.id === playingTrackId}
            draggable={canDrag}
            data-drag-index={canDrag ? index : undefined}
            data-row-index={index}
            tabindex={tabbableIndex === index ? 0 : -1}
            aria-rowindex={grouping ? undefined : index + 2}
            onfocus={() => (focusedIndex = index)}
            onkeydown={(event) => onRowKeydown(event, index)}
            ondblclick={(event) => onRowDoubleClick(event, index)}
            use:menuFor={() =>
              trackMenuItems({
                track: t,
                index,
                trackCount: rows.length,
                favorites,
                blocked: reasons.length > 0,
                play: () => play(index),
                startRadio: () => void playTrackRadio(t.id, toasts),
                addToQueue: () => queueOne(t),
                addToPlaylist: () => onAddToPlaylist?.(t),
                copyTitle: () => void copyText(t.title),
                trackInfo: () => trackInfo.open(t),
                playlist: playlistActions && { ...playlistActions, canMove: !view.engaged },
              })}
          >
            <td class="muted tnum num-cell">{row.number ?? index + 1}</td>
            <td class="thumb-cell">
              <button class="thumb" onclick={() => play(index)} disabled={loadingAll} aria-label="Play" title="Play">
                <Cover uuid={t.album?.cover} size={80} radius={0} alt="" />
                <span class="thumb-play" aria-hidden="true"><Icon name="play" size={14} /></span>
              </button>
            </td>
            <td class="title-cell"><div class="title-text">
              {t.title}
              {#each tags as tag (tag)}
                <span
                  class="badge"
                  class:block-badge={reasons.includes(tag)}
                  class:tag-badge={!reasons.includes(tag)}
                  title={reasons.includes(tag)
                    ? (tag === "explicit" ? "Hidden: explicit" : "Hidden: AI-generated")
                    : (tag === "explicit" ? "Explicit" : "AI-generated")}
                >{tag === "explicit" ? "E" : "AI"}</span>
              {/each}
            </div></td>
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
                onclick={() => queueOne(t)}
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
        {/if}
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
    margin-bottom: 6px;
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
    padding: 8px 12px 8px 0;
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
  .row.blocked .title-text,
  .row.blocked .cell-text {
    opacity: 0.45;
  }
  .row.blocked .title-text {
    text-decoration: line-through;
    text-decoration-color: var(--line);
  }
  .block-badge {
    margin-left: 6px;
    border-color: color-mix(in srgb, var(--warning) 55%, transparent);
    color: var(--warning);
  }
  .tag-badge {
    margin-left: 6px;
    border-color: var(--line);
    color: var(--text-muted);
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
  .section-row th {
    text-align: left;
    padding: 16px 12px 6px 0;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
</style>
