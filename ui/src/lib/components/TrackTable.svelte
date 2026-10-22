<script lang="ts">
  import { tick, untrack } from "svelte";
  import type { Track } from "../api";
  import {
    volumeDisplayRows,
    type VolumeSection,
    type VolumeDisplayRow,
  } from "../albumVolumes";
  import { api, trackToQueueItem } from "../api";
  import { copyText } from "../clipboard";
  import { errorMessage } from "../errors";
  import { formatDuration, formatReleaseDate } from "../format";
  import { locatePlaying } from "../locatePlaying.svelte";
  import { menuFor, type MenuItem } from "../menu.svelte";
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
  import { sortData } from "../sortData.svelte";
  import { trackColumns, type OptionalColumn } from "../trackColumns.svelte";
  import { keysFor, releaseDateOf, SORT_LABELS, type SortKey, type TableKind } from "../trackSort";
  import { trackDrag, type DragParams } from "../drag";
  import { hereTargets, menuArtists } from "../nameLinks";
  import type { Nav } from "../nav.svelte";
  import ArtistLinks from "./ArtistLinks.svelte";
  import NameLink from "./NameLink.svelte";
  import Icon from "./Icon.svelte";
  import Cover from "./Cover.svelte";
  import NowPlayingMark from "./NowPlayingMark.svelte";
  import TrackFilter from "./TrackFilter.svelte";
  import SortBar from "./SortBar.svelte";
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
    compact = false,
    /// Multi-volume album groups (album page); ignored while a sort/filter is
    /// engaged. Grouped lists render whole (no virtualization) and section
    /// headings reset the row number per volume.
    sections,
    playingTrackId = null,
    playbackRunning = false,
    nav,
    kind = "search",
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
    /// A short preview list (search overview): no toolbar, header, number or
    /// quality column; the artists sit under the title, and the album column
    /// shows only while the list is wide enough for it.
    compact?: boolean;
    sections?: VolumeSection[];
    /// Highlights the playing row (accent tint + title, ringed thumb).
    playingTrackId?: number | null;
    /// Playback is running: the playing row's mark spins (still when paused).
    playbackRunning?: boolean;
    /// Makes titles, artists and albums links (name-links spec); without it
    /// they stay plain text.
    nav?: Nav;
    /// Which table this is; decides the sort keys and optional columns it
    /// offers (multi-sort MS-D11/D12).
    kind?: TableKind;
  } = $props();

  const sortKeys = $derived(keysFor(kind));
  const columnStore = trackColumns();
  /// Chosen optional columns plus any the sort uses (MS-D12).
  const shownColumns = $derived.by<OptionalColumn[]>(() => {
    if (compact) return [];
    const chosen = columnStore.chosen(kind);
    return columnStore
      .available(kind)
      .filter((column) => chosen.includes(column) || view.levels.some((level) => level.key === column));
  });

  const COLUMN_TITLES: Record<OptionalColumn, string> = {
    added: "When this track was added",
    released: "The album's release date",
    popularity: "Popularity on TIDAL (0–100), not your own listening",
    plays: "Your plays recorded in listening history on this PC",
  };

  function columnMenu(): MenuItem[] {
    const chosen = columnStore.chosen(kind);
    return columnStore.available(kind).map((column) => ({
      id: `column-${column}`,
      label: SORT_LABELS[column],
      section: "Columns",
      selected: chosen.includes(column),
      icon: chosen.includes(column) ? "check" : undefined,
      run: () => columnStore.toggle(kind, column),
    }));
  }

  function columnText(t: Track, column: OptionalColumn): string {
    switch (column) {
      case "added":
        return formatReleaseDate(t.dateAdded);
      case "released":
        return formatReleaseDate(releaseDateOf(t, sortData.context));
      case "popularity":
        return t.popularity === undefined ? "" : String(t.popularity);
      case "plays":
        return String(sortData.context.plays.get(t.id) ?? "");
    }
  }

  function onSortClick(event: MouseEvent, key: SortKey) {
    if (event.shiftKey) view.addSort(key);
    else view.toggleSort(key);
  }

  function ariaSort(key: SortKey): "ascending" | "descending" | undefined {
    const primary = view.levels[0];
    if (primary?.key !== key) return undefined;
    return primary.dir === "asc" ? "ascending" : "descending";
  }

  /// The page's own album/artist render unlinked (NL-D4).
  const here = $derived(nav ? hereTargets(nav.route) : null);

  function albumOpener(t: Track): (() => void) | undefined {
    const album = t.album;
    if (!nav || !album || album.id === here?.albumId) return undefined;
    return () => nav.goto({ name: "album", id: album.id });
  }

  function openArtist(id: number) {
    nav?.goto({ name: "artist", id });
  }

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
  // The window is read through its bounds, so a scroll that stays inside the
  // same rows (most frames) keeps `range` and everything below it unchanged;
  // a fresh windowRange object every frame re-ran the slice and every row.
  const span = $derived(
    grouping ? null : windowRange(rows.length, scrollTop, viewport, rowHeight, OVERSCAN),
  );
  const first = $derived(span ? span.first : 0);
  const last = $derived(span ? span.last : rows.length);
  const range = $derived({
    first,
    last,
    padTop: grouping ? 0 : first * rowHeight,
    padBottom: grouping ? 0 : Math.max(0, (rows.length - last) * rowHeight),
  });
  const slice = $derived(grouping ? rows : rows.slice(first, last));

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
  /// Flat rows are reused by key while they stay in the window, so moving the
  /// window by a row updates the rows entering and leaving, not all of them.
  let flatRows = new Map<string, VolumeDisplayRow>();
  const displayRows = $derived.by<VolumeDisplayRow[]>(() => {
    if (!grouping) {
      const kept = new Map<string, VolumeDisplayRow>();
      const out = slice.map((track, i) => {
        const index = range.first + i;
        const key = `${track.id}-${index}`;
        let row = flatRows.get(key);
        if (row?.track !== track) row = { key, label: null, track, index, number: index + 1 };
        kept.set(key, row);
        return row;
      });
      flatRows = kept;
      return out;
    }
    return volumeDisplayRows(grouping, rowSourceIndex);
  });
  const columns = $derived(compact ? (showAlbum ? 5 : 4) : (showAlbum ? 8 : 7) + shownColumns.length);

  // Rows render immediately; flags land asynchronously when the AI filter or
  // content labels are on (the store's `ensure` is a no-op when both are off).
  $effect(() => {
    trackFlags.recordTracks(slice);
    void trackFlags.ensure(slice.map((t) => t.id));
  });

  // Plays / release-date columns fill in for the rendered rows; `ensure`
  // skips anything already known. Untracked: it reads the maps it writes.
  $effect(() => {
    const keys = shownColumns.filter((column) => column === "plays" || column === "released");
    if (keys.length === 0) return;
    const visible = slice;
    untrack(() => void sortData.ensure(keys, visible));
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
    offset: () => bodyOffset,
    onChange: (from, at) => {
      dragFrom = from;
      dropAt = at;
    },
    onDrop: (from, to) => playlistActions?.onDrop?.(from, to),
  });

  function blockedNotice(title: string) {
    toasts.show(
      `"${title}" is hidden by your content filters. Change them in Settings → Content.`,
      { severity: "info" },
    );
  }

  /// Add to the end of the queue, or (`next`) right after the playing track.
  function queueOne(t: Track, next = false) {
    if (trackFlags.isBlocked(t)) {
      blockedNotice(t.title);
      return;
    }
    const items = [trackToQueueItem(t)];
    (next ? api.queuePlayNext(items) : api.queueAdd(items)).then(
      () => toasts.show(next ? `"${t.title}" plays next` : `Added "${t.title}" to the queue`),
      (e) => toasts.show(errorMessage(e), { severity: "error" }),
    );
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
    if (target instanceof Element && target.closest("button, a, input, select, textarea, [role='link']")) return;
    void play(index);
  }

  /// Fill mode splits the header into its own table above the scroller, so
  /// the rows can fade out at the scroller's edges (under the header and
  /// above the player bar) instead of passing under a frosted header. The
  /// mask sits on the scroller itself: it stays put while the compositor
  /// scrolls the rows under it, so it never trails a fast scroll and costs
  /// nothing per frame. Both tables use fixed layout over the same colgroup,
  /// so their columns line up without measuring. Only the fade lengths come
  /// from script: each grows in with the distance left to scroll that way (a
  /// list at rest or at its end shows every row whole), so they change in the
  /// first and last EDGE_FADE px of travel only.
  const split = $derived(fill && !compact);
  /// Content above the first row inside the scroller: the sticky head, unless
  /// it was split out.
  const bodyOffset = $derived(split ? 0 : theadHeight);
  const EDGE_FADE = 48;
  let scrollMax = $state(0);
  $effect(() => {
    if (!split) return;
    void [rows.length, viewport, rowHeight];
    const element = scrollEl;
    if (element) scrollMax = element.scrollHeight - element.clientHeight;
  });
  const fadeTop = $derived(split ? Math.min(scrollTop, EDGE_FADE) : 0);
  const fadeBottom = $derived(split ? Math.min(Math.max(0, scrollMax - scrollTop), EDGE_FADE) : 0);
  /// Fixed-layout widths for the content-sized columns: the row number's
  /// digits and the row's action buttons (counts, not measurements).
  const numDigits = $derived(Math.max(2, String(rows.length).length));
  const actionCount = $derived(onAddToPlaylist ? 3 : 2);
  /// The split tables are presentational; the frame is the ARIA table, so
  /// its sections, rows and cells carry their roles explicitly.
  const roleOf = (role: string) => (split ? role : undefined);

  function onScroll(event: Event) {
    const element = event.currentTarget as HTMLElement;
    scrollTop = element.scrollTop;
    if (split) scrollMax = element.scrollHeight - element.clientHeight;
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

  /// The playing track's position in the visible rows; -1 when it isn't
  /// among them (not playing, filtered out, or on a page not loaded yet).
  const playingIndex = $derived(
    playingTrackId === null ? -1 : rows.findIndex((t) => t.id === playingTrackId),
  );

  /// Scroll the playing row to the middle of the visible rows (between the
  /// header and, in fill mode, the player bar clearance). Flat rows sit at
  /// `index * rowHeight` below the header (virtualized, so the row may not be
  /// mounted yet); grouped rows render whole and are measured.
  function scrollToPlaying() {
    const scroll = scrollEl;
    const index = playingIndex;
    if (!scroll || index < 0) return;
    let rowTop = bodyOffset + index * rowHeight;
    if (grouping) {
      const row = scroll.querySelector<HTMLElement>(`tr[data-row-index="${index}"]`);
      if (!row) return;
      rowTop = row.getBoundingClientRect().top - scroll.getBoundingClientRect().top + scroll.scrollTop;
    }
    const clearance = parseFloat(getComputedStyle(scroll).paddingBottom) || 0;
    const middle = bodyOffset + (viewport - clearance - bodyOffset - rowHeight) / 2;
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    scroll.scrollTo({ top: Math.max(0, rowTop - middle), behavior: reduce ? "auto" : "smooth" });
  }

  /// Double-clicking "Now playing" follows the playing track: every time it
  /// changes (next track, or a sort/filter moves it) the list scrolls to it
  /// again. Another double-click stops; it lasts while the table is mounted.
  let following = $state(false);
  $effect(() => {
    if (!following) return;
    void playingIndex;
    untrack(scrollToPlaying);
  });

  // The player bar's "From …" link asks the open page to show the playing
  // track; only requests made while this table is mounted count.
  let locateSeen = locatePlaying.requests;
  $effect(() => {
    const requests = locatePlaying.requests;
    if (requests === locateSeen) return;
    locateSeen = requests;
    untrack(scrollToPlaying);
  });

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

{#snippet sortHead(key: SortKey, label: string, align: "left" | "right", extraClass = "")}
  {@const rank = view.levels.findIndex((level) => level.key === key)}
  {#if sortKeys.includes(key)}
    <th
      scope="col"
      role={roleOf("columnheader")}
      class="sort-head {extraClass}"
      class:active={rank >= 0}
      style="text-align:{align}"
      aria-sort={ariaSort(key)}
      title={key in COLUMN_TITLES ? COLUMN_TITLES[key as OptionalColumn] : undefined}
    >
      <span class="head-inner" class:end={align === "right"}>
        <button
          class="sort"
          disabled={view.preparing}
          title="Sort by {label} (Shift+click to add as the next level)"
          onclick={(event) => onSortClick(event, key)}
        >
          {label}
          {#if rank >= 0}
            <span class="rank-mark" aria-hidden="true">
              {#if view.levels.length > 1}<span class="rank">{rank + 1}</span>{/if}
              <Icon name={view.levels[rank].dir === "asc" ? "arrow-up" : "arrow-down"} size={12} />
            </span>
          {/if}
        </button>
        {#if view.levels.length > 0 && rank < 0}
          <button
            class="add-level"
            disabled={view.preparing}
            aria-label="Then sort by {label}"
            title="Then sort by {label}"
            onclick={() => view.addSort(key)}
          ><Icon name="plus" size={12} /></button>
        {/if}
      </span>
    </th>
  {:else}
    <th scope="col" role={roleOf("columnheader")} class={extraClass} style="text-align:{align}">{label}</th>
  {/if}
{/snippet}

{#snippet headRow()}
  <tr use:menuFor={columnMenu} role={roleOf("row")}>
    <th scope="col" role={roleOf("columnheader")} class="num-head" style="text-align:right">
      <button class="sort" title="Original order" onclick={() => view.clearSort()}>#</button>
    </th>
    <th scope="col" role={roleOf("columnheader")} class="thumb-head"></th>
    {@render sortHead("title", "Title", "left")}
    {@render sortHead("artist", "Artist", "left")}
    {#if showAlbum}{@render sortHead("album", "Album", "left")}{/if}
    {#each shownColumns as column (column)}
      {@render sortHead(column, SORT_LABELS[column], column === "added" || column === "released" ? "left" : "right", "opt-col")}
    {/each}
    {@render sortHead("duration", "Time", "left")}
    {@render sortHead("quality", "Quality", "left")}
    <th scope="col" role={roleOf("columnheader")} class="actions-head"></th>
  </tr>
{/snippet}

<!-- Split mode: both tables size their columns from this, in fixed layout. -->
{#snippet columnSizes()}
  <colgroup>
    <col class="col-num" />
    <col class="col-thumb" />
    <col class="col-title" />
    <col class="col-text" />
    {#if showAlbum}<col class="col-album" />{/if}
    {#each shownColumns as column (column)}<col class="col-opt opt-col" />{/each}
    <col class="col-time" />
    <col class="col-quality" />
    <col class="col-actions" />
  </colgroup>
{/snippet}

{#if !compact}
  <div class="table-tools">
    {#if showFilter}<TrackFilter {view} />{/if}
    <SortBar {view} keys={sortKeys} />
    {#if playingIndex >= 0}
      <button
        class="locate"
        class:following
        type="button"
        aria-pressed={following}
        title={following
          ? "Following the current track (double-click to stop)"
          : "Show the current track (double-click to keep following it)"}
        onclick={scrollToPlaying}
        ondblclick={() => (following = !following)}
      >
        <Icon name="circle-dot" size={14} /> {following ? "Following" : "Now playing"}
      </button>
    {/if}
  </div>
{/if}

{#snippet body()}
<div
  class="table-scroll"
  class:pane={!split}
  class:pane-flush={!split}
  class:compact
  class:fill
  class:scrolled
  bind:this={scrollEl}
  style:max-height={fill ? "none" : maxHeight}
  style:--fade-top={split ? `${fadeTop}px` : undefined}
  style:--fade-bottom={split ? `${fadeBottom}px` : undefined}
  bind:clientHeight={viewport}
  onscroll={onScroll}
  data-perf-scroll
  use:trackDrag={drag}
>
  {#if !compact && !fill}
  <div
    class="head-frost"
    aria-hidden="true"
    style="height:{theadHeight}px;margin-bottom:-{theadHeight}px"
  ></div>
  {/if}
  <table
    role={roleOf("presentation")}
    aria-label={split ? undefined : "Tracks"}
    aria-rowcount={split || grouping || compact ? undefined : rows.length + 1}
  >
    {#if split}
      {@render columnSizes()}
    {:else if !compact}
      <thead bind:clientHeight={theadHeight}>{@render headRow()}</thead>
    {/if}
    <tbody role={roleOf("rowgroup")}>
      {#if range.padTop > 0}
        <tr aria-hidden="true"><td colspan={columns} style="height:{range.padTop}px;padding:0;border:0"></td></tr>
      {/if}
      {#each displayRows as row (row.key)}
        {#if row.label !== null}
          <tr class="section-row" role={roleOf("row")}>
            <th scope="colgroup" role={roleOf("columnheader")} colspan={columns} aria-colspan={split ? columns : undefined}>{row.label}</th>
          </tr>
        {:else if row.track !== undefined}
          {@const t = row.track}
          {@const index = row.index ?? 0}
          {@const quality = trackQualityLabel(t.audioQuality)}
          {@const reasons = trackFlags.blockReasons(t)}
          {@const tags = trackFlags.showLabels
            ? [...new Set([...reasons, ...trackFlags.labels(t)])]
            : reasons}
          {@const openAlbum = albumOpener(t)}
          <tr
            class="row"
            role={roleOf("row")}
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
                favorites,
                blocked: reasons.length > 0,
                play: () => play(index),
                startRadio: () => void playTrackRadio(t.id, toasts, t.title),
                playNext: () => queueOne(t, true),
                addToQueue: () => queueOne(t),
                addToPlaylist: () => onAddToPlaylist?.(t),
                copyTitle: () => void copyText(t.title),
                trackInfo: () => trackInfo.open(t),
                goToAlbum: openAlbum,
                goToArtists: nav
                  ? menuArtists(t.artists, here).map((a) => ({ name: a.name, open: () => openArtist(a.id) }))
                  : undefined,
                playlist: playlistActions,
              })}
          >
            {#if !compact}
              <td class="muted tnum num-cell" role={roleOf("cell")}>
                {#if playingTrackId !== null && t.id === playingTrackId}
                  <span class="num-mark"><NowPlayingMark playing={playbackRunning} size={16} label={playbackRunning ? "Now playing" : "Paused"} /></span>
                {:else}
                  {row.number ?? index + 1}
                {/if}
              </td>
            {/if}
            <td class="thumb-cell" role={roleOf("cell")}>
              <button class="thumb" onclick={() => play(index)} disabled={loadingAll} aria-label="Play" title="Play">
                <Cover uuid={t.album?.cover} size={80} radius={0} alt="" />
                <span class="thumb-play" aria-hidden="true"><Icon name="play" size={14} /></span>
              </button>
            </td>
            <td class="title-cell" role={roleOf("cell")}><div class="title-text">
              <NameLink label={t.title} onOpen={openAlbum} focusable={false} />
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
            </div>
            {#if compact}
              <div class="cell-text sub-text muted">
                <ArtistLinks
                  artists={t.artists}
                  onOpen={nav ? openArtist : undefined}
                  current={here?.artistId}
                  focusable={false}
                />
              </div>
            {/if}
            </td>
            {#if !compact}
            <td class="text-cell muted" role={roleOf("cell")}><div class="cell-text">
              <ArtistLinks
                artists={t.artists}
                onOpen={nav ? openArtist : undefined}
                current={here?.artistId}
                focusable={false}
              />
            </div></td>
            {/if}
            {#if showAlbum}
              <td class="text-cell muted album-cell" role={roleOf("cell")}><div class="cell-text">
                <NameLink label={t.album?.title ?? ""} onOpen={openAlbum} focusable={false} />
              </div></td>
            {/if}
            {#each shownColumns as column (column)}
              <td
                role={roleOf("cell")}
                class="muted nowrap-cell opt-col"
                class:tnum={column === "popularity" || column === "plays"}
                class:num-col={column === "popularity" || column === "plays"}
              >{columnText(t, column)}</td>
            {/each}
            <td class="muted tnum nowrap-cell time-cell" role={roleOf("cell")}>{formatDuration(t.duration)}</td>
            {#if !compact}
              <td class="nowrap-cell quality-cell" role={roleOf("cell")}>
                {#if quality}<span class="badge" class:max={quality === "MAX"}>{quality}</span>{/if}
              </td>
            {/if}
            <td class="actions-cell" role={roleOf("cell")}>
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
                class:reveal={kind === "favorites" && favorites.hasTrack(t.id)}
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
        <tr role={roleOf("row")}><td colspan={columns} role={roleOf("cell")} aria-colspan={split ? columns : undefined} class="muted">{view.filter.trim() ? `No tracks match “${view.filter.trim()}”` : "No tracks"}</td></tr>
      {/if}
    </tbody>
  </table>
  {#if dropAt !== null}
    <div class="drop-line" style="top:{bodyOffset + dropAt * rowHeight}px"></div>
  {/if}
</div>
{/snippet}

{#if split}
  <div
    class="table-frame pane pane-flush"
    role="table"
    aria-label="Tracks"
    aria-rowcount={grouping ? undefined : rows.length + 1}
    style:--num-digits={numDigits}
    style:--actions={actionCount}
  >
    <div class="table-head">
      <table role="presentation">
        {@render columnSizes()}
        <thead role={roleOf("rowgroup")} bind:clientHeight={theadHeight}>{@render headRow()}</thead>
      </table>
    </div>
    {@render body()}
  </div>
{:else}
  {@render body()}
{/if}

<style>
  .table-tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    margin-bottom: 6px;
  }
  /* Same look as the Sort button beside it (SortBar's `.quiet`): a soft
     resting fill, a raised fill + line and an accent icon on hover. While
     following it takes the accent like an active sort chip. */
  .locate {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 12px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--surface-2) 55%, transparent);
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
    transition:
      color var(--dur-micro) var(--ease-enter),
      background var(--dur-micro) var(--ease-enter),
      border-color var(--dur-micro) var(--ease-enter),
      transform var(--dur-micro) var(--ease-enter);
  }
  .locate :global(svg) {
    transition: color var(--dur-micro) var(--ease-enter);
  }
  .locate:hover {
    color: var(--text);
    background: var(--surface-2);
    border-color: var(--line);
  }
  .locate:hover :global(svg),
  .locate.following :global(svg) {
    color: var(--accent);
  }
  .locate:active {
    transform: scale(0.97);
  }
  .locate.following {
    color: var(--text);
    border-color: color-mix(in srgb, var(--accent) 55%, var(--line));
    background: color-mix(in srgb, var(--accent) 16%, var(--surface-2));
  }
  /* A container, so the optional columns drop out on narrow widths in CSS
     (the queue glide needs width-driven layout in CSS, not JS counts). */
  .table-scroll {
    overflow: auto;
    position: relative;
    container-type: inline-size;
  }
  @container (max-width: 760px) {
    .opt-col {
      display: none;
    }
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
  /* Split (fill) mode: the frame is the pane and the width container (the
     header table sits outside the scroller but drops the same optional
     columns); the scroller takes what the header leaves. */
  /* `--scrollbar-w`: the standard thin bar's width (app.css), which the edge
     fade leaves unmasked. */
  .table-frame {
    --scrollbar-w: 10px;
    flex: 1 1 auto;
    min-height: 140px;
    display: flex;
    flex-direction: column;
    container-type: inline-size;
  }
  .table-frame .table-scroll {
    container-type: normal;
    min-height: 0;
  }
  /* Both parts reserve the scrollbar's gutter, so the header table is
     exactly as wide as the body table whether or not the list scrolls. */
  .table-head,
  .table-frame .table-scroll {
    scrollbar-gutter: stable;
  }
  .table-head {
    flex: none;
    overflow: hidden;
  }
  .table-frame table {
    table-layout: fixed;
  }
  /* The static edge fade (see `split` in the script): rows fade out under
     the header once scrolled and before the player bar while there is more
     below; past the bar's top edge they are hidden. A second layer keeps the
     scrollbar strip unmasked. Only the fade lengths vary, near the ends. */
  @property --fade-top {
    syntax: "<length>";
    inherits: false;
    initial-value: 0px;
  }
  @property --fade-bottom {
    syntax: "<length>";
    inherits: false;
    initial-value: 0px;
  }
  .table-frame .table-scroll {
    mask:
      linear-gradient(
          to bottom,
          transparent 0,
          #000 var(--fade-top),
          #000 calc(100% - var(--player-height) - 8px - var(--fade-bottom)),
          transparent calc(100% - var(--player-height) - 8px)
        )
        left top / calc(100% - var(--scrollbar-w)) 100% no-repeat,
      linear-gradient(#000, #000) right top / var(--scrollbar-w) 100% no-repeat;
  }
  /* Fixed-layout columns. The number, cover and actions are sized from
     their content (digit and button counts from the script); the shares
     split the rest by ratio. They sum past 100% whatever columns show, which
     makes Chromium keep the px columns and scale the shares to fit (below
     100% it would widen the px columns; calc(% - px) degrades to an equal
     split). Ratios match the auto-layout shares below. */
  .col-num {
    width: calc(var(--num-digits) * 1ch + 22px);
  }
  .col-thumb {
    width: calc(var(--row-height) - 1px + 18px);
  }
  .col-actions {
    width: calc(var(--actions) * 32px - 6px);
  }
  .col-title {
    width: 42%;
  }
  .col-text {
    width: 38%;
  }
  .col-album {
    width: 36%;
  }
  .col-opt {
    width: 28%;
  }
  .col-time,
  .col-quality {
    width: 16%;
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
  /* Separate borders: the row rules belong to the cells (with the rows'
     own height math), not to the table. */
  table {
    width: 100%;
    border-collapse: separate;
    border-spacing: 0;
  }
  /* One frost layer for the sticky header (tables that don't fill the page;
     fill mode splits the header out and fades its rows instead). No
     per-cell backdrop-filter and no mask on the frost: both are unreliable
     in Chromium inside a scroller (masked blur can
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
    transition: box-shadow var(--dur-state) var(--ease-enter);
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
    transition: opacity var(--dur-state) var(--ease-enter);
  }
  .table-scroll.scrolled .head-frost::after {
    opacity: 1;
  }
  thead th {
    position: sticky;
    top: 0;
    z-index: 2;
    background: transparent;
    transition: background-color var(--dur-state) var(--ease-enter);
    padding: 8px 12px 8px 0;
  }
  .table-scroll.scrolled thead th {
    background: var(--glass-veil);
  }
  thead th.thumb-head {
    padding: 0;
  }
  .sort {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: transparent;
    border: 0;
    color: inherit;
    padding: 0;
    cursor: pointer;
    white-space: nowrap;
  }
  .head-inner {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .head-inner.end {
    flex-direction: row-reverse;
  }
  .sort-head.active .sort {
    color: var(--accent);
  }
  .rank-mark {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .rank {
    display: inline-grid;
    place-items: center;
    min-width: 14px;
    height: 14px;
    border-radius: 7px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 9px;
    font-weight: 700;
  }
  /* "Then sort by": shown on header hover or keyboard focus only. */
  .add-level {
    display: inline-grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--text-muted);
    opacity: 0;
    transition: opacity var(--dur-micro);
  }
  .sort-head:hover .add-level,
  .add-level:focus-visible {
    opacity: 1;
  }
  .add-level:hover {
    color: var(--accent);
    border-color: var(--accent);
  }
  .num-col {
    text-align: right;
  }
  .row {
    height: var(--row-height);
    user-select: none;
    transition: background-color var(--dur-micro);
  }
  .row:hover {
    background: var(--surface-2);
  }
  td {
    padding: 0 12px 0 0;
  }
  .row > td {
    border-bottom: 1px solid var(--line);
  }
  /* The cover fills the row top to bottom; -1px keeps it inside the row's
     content box so the virtualization row math (var(--row-height)) holds. */
  .thumb-cell {
    padding: 0 18px 0 0;
  }
  /* The number, cover and title get more air than the text columns, so the
     row's start doesn't read as one block. */
  .num-cell,
  thead th.num-head {
    padding-right: 18px;
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
  /* Every column but the number, cover and actions takes a share of the
     width, so the date, time and quality spread across the row instead of
     packing at the right edge (title a little wider than artist and album).
     The number, cover and actions take a token 1% so they shrink to their
     content: a table without an album or optional columns then hands its
     spare width to the shared columns by their share (a px width would make
     them fixed columns, which Chromium widens first). */
  .title-cell {
    width: 21%;
  }
  .text-cell {
    width: 19%;
  }
  .album-cell {
    width: 18%;
  }
  .opt-col {
    width: 14%;
  }
  .time-cell,
  .quality-cell {
    width: 8%;
  }
  .num-cell,
  thead th.num-head,
  .thumb-cell,
  thead th.thumb-head,
  .actions-cell {
    width: 1%;
  }
  /* Time starts its column like the other shared columns, so the gap to the
     quality badge is the column's width, not one cell padding. The compact
     list keeps it at the row's end. */
  .num-cell,
  .compact .time-cell {
    text-align: right;
  }
  /* The playing row's number gives way to the spinning mark, right-aligned
     with the digits. */
  .num-mark {
    display: inline-flex;
    vertical-align: middle;
  }
  /* A long name ends in an ellipsis (the full name is the link's tooltip)
     instead of widening its column: inline-size containment keeps the text
     out of the cell's intrinsic width, which a table would otherwise grow
     to fit. */
  .title-text,
  .cell-text {
    contain: inline-size;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .nowrap-cell {
    white-space: nowrap;
  }
  .text-cell :global(.name-link:hover) {
    color: var(--text);
  }
  .actions-cell {
    padding: 0;
    white-space: nowrap;
    text-align: right;
  }
  /* Explicit (it is the UA default): `.col-actions` counts 26px per button. */
  .actions-cell button {
    padding: 1px 6px;
  }
  .actions-cell button + button {
    margin-left: 6px;
  }
  /* Queue/playlist actions hide until the row is hovered or focused; the
     heart stays visible except on the favorites list, where every row is a
     favorite (an unfavorited row keeps its outline heart). Pointer-events
     follow so hidden buttons can't be hit. */
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
  /* Compact: a list, not a table. No rules between rows; the title carries
     the artists on a second line and shares the width with the album, which
     drops out when the list is narrow (a container query, so the queue glide
     sees the final layout). */
  /* Taller rows with an inset, rounded cover so the thumbs don't touch. The
     row math stays at --row-height: a preview renders whole, never windowed
     (it is far shorter than the overscan). */
  .compact .row {
    height: calc(var(--row-height) + 10px);
  }
  .compact .row > td {
    border-bottom: 0;
  }
  .compact .thumb-cell {
    padding: 0 14px 0 6px;
  }
  .compact .thumb {
    width: calc(var(--row-height) - 6px);
    height: calc(var(--row-height) - 6px);
    overflow: hidden;
    border-radius: 4px;
  }
  .compact .title-cell {
    width: 55%;
    line-height: 1.25;
  }
  .compact .album-cell {
    width: 45%;
    padding-right: 24px;
  }
  .compact .time-cell {
    width: 1%;
  }
  @container (max-width: 640px) {
    .compact .album-cell {
      display: none;
    }
    .compact .title-cell {
      width: 100%;
    }
  }
  .sub-text {
    font-size: 12.5px;
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
