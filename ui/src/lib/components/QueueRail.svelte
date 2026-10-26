<script lang="ts">
  import { untrack } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, type TransitionConfig } from "svelte/transition";
  import { api, trackToQueueItem, type QueueItem } from "../api";
  import { copyText } from "../clipboard";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import Icon from "./Icon.svelte";
  import Cover from "./Cover.svelte";
  import NowPlayingMark from "./NowPlayingMark.svelte";
  import { appearance } from "../appearance.svelte";
  import { formatDuration, formatListenTime } from "../format";
  import { playSource } from "../playSource.svelte";
  import { queueRowHeightFor } from "../rowHeight";
  import { DRAG_THRESHOLD, dragTarget, edgeScroll, rowShift } from "../queueDrag";
  import VirtualList from "./VirtualList.svelte";
  import { menuFor } from "../menu.svelte";
  import { queueMenuItems } from "../menus";
  import type { Player } from "../player.svelte";
  import type { Route } from "../nav.svelte";
  import { albumRoute, goToTargets } from "../queueLinks";
  import { dragDrop, dropZone, type DropZone } from "../dragDrop.svelte";
  import { resolveTracks } from "../dragPayload";
  import { errorMessage } from "../errors";
  import { glideActive } from "../queueTransition";
  import type { Toasts } from "../toasts.svelte";
  import { trackFlags } from "../trackFlags.svelte";
  import NameLink from "./NameLink.svelte";
  import QueueArtistLinks from "./QueueArtistLinks.svelte";

  let {
    player,
    title = true,
    go,
    toasts,
  }: {
    player: Player;
    /// Rack shells draw their own unit heading; they pass false.
    title?: boolean;
    /// Opens a page from a row's names (name-links spec); without it the names
    /// stay plain text (mini players).
    go?: (route: Route) => void;
    /// Reports drops on the rail; without it they land silently.
    toasts?: Toasts;
  } = $props();

  /// A name click right after a drag release must not navigate, just as the
  /// row's play click is swallowed.
  function openFromRow(route: Route) {
    if (!swallowClick) go?.(route);
  }

  async function openAlbum(item: QueueItem) {
    const route = await albumRoute(item, api.getTrack);
    if (route) openFromRow(route);
  }

  let confirmingClear = $state(false);
  let scroller = $state<HTMLElement | null>(null);
  let rail = $state<HTMLElement | null>(null);

  const rowH = $derived(queueRowHeightFor(appearance.density));

  // Optimistic order: a drop or keyboard move shows the new order at once and
  // the next engine state (any new queue array) replaces it.
  let local = $state<{ items: QueueItem[]; current: number | null } | null>(null);
  $effect(() => {
    void player.state.queue;
    local = null;
  });

  const items = $derived(local?.items ?? player.state.queue);
  /// The engine's queue position, so a track queued twice lights up once.
  const currentIndex = $derived.by(() => {
    if (local) return local.current;
    const index = player.state.current_index;
    if (index === null || index === undefined || !player.state.current) return null;
    return index < items.length ? index : null;
  });

  const upcoming = $derived(currentIndex === null ? items.length : items.length - currentIndex - 1);
  const upcomingSecs = $derived.by(() => {
    let total = 0;
    for (let i = currentIndex === null ? 0 : currentIndex + 1; i < items.length; i++) {
      total += items[i].duration_secs;
    }
    return total;
  });
  const remainingMs = $derived(
    upcomingSecs * 1000 +
      (currentIndex === null ? 0 : Math.max(0, player.state.duration_ms - player.state.position_ms)),
  );

  function moved(from: number, to: number) {
    const next = items.slice();
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    // Same rule as the engine's Queue::reorder.
    let current = currentIndex;
    if (current !== null) {
      if (current === from) current = to;
      else if (from < current && to >= current) current -= 1;
      else if (from > current && to <= current) current += 1;
    }
    return { items: next, current };
  }

  function reorder(from: number, to: number) {
    if (from === to || to < 0 || to >= items.length) return;
    local = moved(from, to);
    void Promise.resolve(player.reorder(from, to)).catch(() => (local = null));
  }

  function jump(index: number) {
    if (index >= 0 && index < items.length) void api.playTracks(items, index);
  }

  /// "Play next" for a queued row: move it to right after the current track.
  function nextSlot(index: number): number | null {
    if (currentIndex === null || index === currentIndex) return null;
    const to = index > currentIndex ? currentIndex + 1 : currentIndex;
    return to === index ? null : to;
  }

  // ---- Scrolling ----------------------------------------------------------

  /// Bottom of the visible list: the docked rail runs behind the floating
  /// player bar, and the scroller's bottom padding minus its fixed tail
  /// (TAIL) is that overlap.
  function visibleBottom(el: HTMLElement): number {
    const padding = parseFloat(getComputedStyle(el).paddingBottom) || 0;
    return el.getBoundingClientRect().bottom - Math.max(0, padding - TAIL);
  }

  /// Breathing room (px) above and below the current row, setting it apart
  /// from the played and upcoming tracks. The virtual list stays uniform:
  /// rows from the current one down are shifted visually (rowTransform), and
  /// the list's bottom padding holds the extra 2 × GAP. Gaps close while a
  /// drag runs so the drop math stays uniform.
  const GAP = 28;
  /// Room above the first upcoming Autoplay suggestion, for its label.
  const AUTOPLAY_GAP = 26;
  /// Bottom padding beyond the player-bar overlap: 12px tail + the gaps.
  const TAIL = 12 + 2 * GAP;

  /// The first Autoplay suggestion still to come; the label sits above it.
  const autoplayFrom = $derived.by(() => {
    for (let i = currentIndex === null ? 0 : currentIndex + 1; i < items.length; i++) {
      if (items[i].autoplay) return i;
    }
    return null;
  });

  function gapBefore(index: number): number {
    if (drag || dropSlot !== null) return 0;
    let gap = currentIndex === null || index < currentIndex ? 0 : index === currentIndex ? GAP : 2 * GAP;
    if (autoplayFrom !== null && index >= autoplayFrom) gap += AUTOPLAY_GAP;
    return gap;
  }

  function rowTop(index: number): number {
    return index * rowH + gapBefore(index);
  }

  function rowInView(index: number): boolean {
    const el = scroller;
    if (!el) return false;
    const top = rowTop(index) - el.scrollTop;
    const height = visibleBottom(el) - el.getBoundingClientRect().top;
    return top >= -rowH / 2 && top + rowH / 2 <= height;
  }

  /// Bring a row near the top, one row of context above it.
  function scrollToRow(index: number, smooth = false) {
    const el = scroller;
    if (!el) return;
    const top = Math.max(0, rowTop(index) - rowH - GAP);
    if (smooth && !appearance.reducedMotion) el.scrollTo({ top, behavior: "smooth" });
    else el.scrollTop = top;
  }

  // Open on the current track; afterwards follow track changes only while the
  // old current row was in view (the user was watching it, not browsing).
  // A fresh scroller (first open, or the list remounting after an empty
  // queue) counts as opening.
  let followed: { el: HTMLElement; id: number | null; index: number } | null = null;
  $effect(() => {
    const el = scroller;
    const index = currentIndex;
    const id = player.state.current?.track_id ?? null;
    if (!el || index === null) return;
    untrack(() => {
      if (followed?.el !== el) scrollToRow(index);
      else if (id !== followed.id && drag === null && rowInView(followed.index)) {
        scrollToRow(index, true);
      }
      followed = { el, id, index };
    });
  });

  // ---- App drags dropped on the rail --------------------------------------

  /// The slot an app drag over the rail would insert at (the row gaps close
  /// meanwhile, so rows sit at index × rowH), and where its line is drawn.
  let dropSlot = $state<number | null>(null);
  let dropLineY = $state(0);
  let listEl = $state<HTMLElement | null>(null);

  function slotAt(clientY: number): number {
    if (!scroller) return items.length;
    return Math.max(0, Math.min(items.length, Math.round(contentY(clientY) / rowH)));
  }

  /// Tracks dropped on the rail (drag-to-playlist spec DD-D8): inserted at
  /// the slot, or appended below the last row, without hidden tracks.
  async function dropTracks(payload: Parameters<typeof resolveTracks>[0], slot: number) {
    try {
      const prepared = await trackFlags.filterPlayable(await resolveTracks(payload), 0);
      if (!prepared) {
        toasts?.show("Everything here is hidden by your content filters.", { severity: "info" });
        return;
      }
      const queued = prepared.items.map(trackToQueueItem);
      if (slot >= items.length) await api.queueAdd(queued);
      else await api.queueInsert(slot, queued);
      const noun = queued.length === 1 ? `"${queued[0].title}"` : `${queued.length} tracks`;
      toasts?.show(`Added ${noun} to the queue`);
    } catch (e) {
      toasts?.show(errorMessage(e), { severity: "error" });
    }
  }

  const queueZone: DropZone = {
    key: "queue",
    accepts: () =>
      glideActive() ? { ok: false, reason: "The queue is moving" } : { ok: true, hint: "Add to queue" },
    hover: (_payload, point) => {
      const slot = items.length === 0 ? 0 : slotAt(point.y);
      dropSlot = slot;
      if (scroller && listEl) {
        const offset = scroller.getBoundingClientRect().top - listEl.getBoundingClientRect().top;
        dropLineY = offset + slot * rowH - scroller.scrollTop;
      }
    },
    leave: () => {
      dropSlot = null;
    },
    drop: (payload, point) => dropTracks(payload, items.length === 0 ? 0 : slotAt(point.y)),
    scroller: () => scroller,
  };

  // ---- Pointer drag -------------------------------------------------------

  interface Press {
    index: number;
    x: number;
    y: number;
    grab: number;
    left: number;
    width: number;
    railTop: number;
  }
  let press = $state.raw<Press | null>(null);
  let drag = $state<{ from: number; to: number; y: number } | null>(null);
  let settling = $state(false);
  let swallowClick = false;
  let frame = 0;

  function contentY(clientY: number): number {
    const el = scroller!;
    return clientY - el.getBoundingClientRect().top + el.scrollTop;
  }

  function retarget() {
    if (!drag || !press || !scroller) return;
    const center = contentY(drag.y) - press.grab + rowH / 2;
    drag.to = dragTarget(center, rowH, items.length);
  }

  function autoScroll() {
    frame = 0;
    if (!drag || !scroller) return;
    const top = scroller.getBoundingClientRect().top;
    const speed = edgeScroll(drag.y, top, visibleBottom(scroller));
    if (speed !== 0) {
      scroller.scrollTop += speed;
      retarget();
    }
    frame = requestAnimationFrame(autoScroll);
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0 || event.pointerType === "touch" || !rail) return;
    const target = event.target as Element;
    if (target.closest(".remove")) return;
    const row = target.closest<HTMLElement>("[data-q-index]");
    if (!row) return;
    const rect = row.getBoundingClientRect();
    const railRect = rail.getBoundingClientRect();
    press = {
      index: Number(row.dataset.qIndex),
      x: event.clientX,
      y: event.clientY,
      grab: event.clientY - rect.top,
      left: rect.left - railRect.left,
      width: rect.width,
      railTop: railRect.top,
    };
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", cancelDrag);
    window.addEventListener("keydown", onDragKey, true);
  }

  function onPointerMove(event: PointerEvent) {
    if (!press) return;
    if (!drag) {
      if (Math.hypot(event.clientX - press.x, event.clientY - press.y) < DRAG_THRESHOLD) return;
      drag = { from: press.index, to: press.index, y: event.clientY };
      document.body.classList.add("queue-dragging");
      frame = requestAnimationFrame(autoScroll);
    }
    drag.y = event.clientY;
    retarget();
  }

  function onPointerUp() {
    const done = drag;
    endDrag();
    if (!done) return;
    // The release still clicks the row underneath; that click must not play.
    swallowClick = true;
    setTimeout(() => (swallowClick = false));
    // Rows hold their shifted transforms; snap (no glide) into the new order.
    settling = true;
    requestAnimationFrame(() => requestAnimationFrame(() => (settling = false)));
    reorder(done.from, done.to);
  }

  function onDragKey(event: KeyboardEvent) {
    if (event.key !== "Escape" || !drag) return;
    event.preventDefault();
    event.stopPropagation();
    cancelDrag();
  }

  function cancelDrag() {
    const wasDragging = drag !== null;
    endDrag();
    if (wasDragging) {
      swallowClick = true;
      setTimeout(() => (swallowClick = false));
    }
  }

  function endDrag() {
    press = null;
    drag = null;
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    document.body.classList.remove("queue-dragging");
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", cancelDrag);
    window.removeEventListener("keydown", onDragKey, true);
  }

  $effect(() => {
    const el = scroller;
    if (!el) return;
    // Covers are <img>s: a native image drag would steal the pointer (and
    // fire pointercancel) mid-reorder.
    const noNativeDrag = (event: DragEvent) => event.preventDefault();
    el.addEventListener("pointerdown", onPointerDown);
    el.addEventListener("dragstart", noNativeDrag);
    return () => {
      el.removeEventListener("pointerdown", onPointerDown);
      el.removeEventListener("dragstart", noNativeDrag);
      endDrag();
    };
  });

  function rowTransform(index: number): string | undefined {
    let y = gapBefore(index);
    if (drag) {
      const shift = index === drag.from ? drag.to - drag.from : rowShift(index, drag.from, drag.to);
      y += shift * rowH;
    }
    return y === 0 ? undefined : `translateY(${y}px)`;
  }

  // ---- Keyboard -----------------------------------------------------------

  /// Roving tab stop: one row in the tab order, arrows move between rows.
  let focusIndex = $state<number | null>(null);
  const tabIndex = $derived(
    Math.min(focusIndex ?? currentIndex ?? 0, Math.max(0, items.length - 1)),
  );

  function focusRow(index: number) {
    if (index < 0 || index >= items.length) return;
    focusIndex = index;
    if (!rowInView(index)) scrollToRow(index);
    // Let the virtual window render the row before focusing it.
    requestAnimationFrame(() =>
      requestAnimationFrame(() =>
        scroller?.querySelector<HTMLElement>(`[data-q-index="${index}"] .row-main`)?.focus(),
      ),
    );
  }

  function onRowKey(event: KeyboardEvent, index: number) {
    const step = event.key === "ArrowUp" ? -1 : event.key === "ArrowDown" ? 1 : 0;
    if (step !== 0 && event.altKey) {
      event.preventDefault();
      if (index + step < 0 || index + step >= items.length) return;
      reorder(index, index + step);
      focusRow(index + step);
    } else if (step !== 0) {
      event.preventDefault();
      focusRow(index + step);
    } else if (event.key === "Delete") {
      event.preventDefault();
      void player.remove(index);
      focusRow(Math.min(index, items.length - 2));
    }
  }

  /// The disc arrives as the row's content slides aside: it starts a beat
  /// late, small and a quarter-turn back, and rolls into place.
  function discIn(_node: Element, { reduced }: { reduced: boolean }): TransitionConfig {
    if (reduced) return { duration: 120, css: (t) => `opacity: ${t}` };
    return {
      delay: 90,
      duration: 320,
      easing: cubicOut,
      css: (t) => `opacity: ${t}; transform: scale(${0.55 + 0.45 * t}) rotate(${(t - 1) * 90}deg)`,
    };
  }

  function onRowClick(index: number) {
    if (swallowClick) return;
    jump(index);
  }
</script>

{#snippet rowBody(item: QueueItem, links: boolean)}
  <span class="thumb">
    <Cover uuid={item.cover} size={80} radius={6} alt="" />
  </span>
  <span class="text">
    {#if links && go}
      <!-- Nested links: the row is the play button (name-links spec NL-D10). -->
      <span class="title"><NameLink label={item.title} onOpen={() => void openAlbum(item)} nested /></span>
      <span class="artist"><QueueArtistLinks {item} go={openFromRow} nested /></span>
    {:else}
      <span class="title">{item.title}</span>
      <span class="artist">{item.artist}</span>
    {/if}
  </span>
{/snippet}

<div
  class="queue-rail"
  class:settling
  class:dragging={drag !== null || dropSlot !== null}
  class:drop-over={dragDrop.targetKey === "queue" && dragDrop.verdict?.ok === true}
  bind:this={rail}
  style:--q-row="{rowH}px"
  use:dropZone={queueZone}
>
  <header class="q-head">
    {#if title}<h2 class="q-title">Queue</h2>{/if}
    {#if items.length > 0}
      <span class="q-meta tnum">
        {#if currentIndex === null}
          {items.length} {items.length === 1 ? "track" : "tracks"} · {formatListenTime(upcomingSecs * 1000)}
        {:else}
          {upcoming} up next · {formatListenTime(remainingMs)} left
        {/if}
      </span>
    {/if}
    <div class="q-tools">
      <button
        class="q-tool"
        onclick={() => currentIndex !== null && scrollToRow(currentIndex, true)}
        disabled={currentIndex === null}
        aria-label="Show the current track"
        title="Show the current track"
      ><Icon name="circle-dot" size={16} /></button>
      <button
        class="q-tool"
        onclick={() => (confirmingClear = true)}
        disabled={items.length === 0}
        aria-label="Clear queue"
        title="Clear queue"
      ><Icon name="trash-2" size={16} /></button>
    </div>
  </header>

  {#if items.length === 0}
    <div class="q-empty">
      <Icon name="list-music" size={28} />
      <p>Your queue is empty</p>
      <p class="muted">Play something, or use “Play next” and “Add to queue” on any track.</p>
    </div>
  {:else}
    <div class="q-list" bind:this={listEl}>
      {#if dropSlot !== null && dragDrop.verdict?.ok}
        <div class="q-drop-line" style:top="{dropLineY}px" aria-hidden="true"></div>
      {/if}
      <VirtualList
        total={items.length}
        rowHeight={rowH}
        maxHeight="none"
        bottomPadding="calc(var(--queue-clear, var(--player-height)) + {TAIL + (autoplayFrom === null ? 0 : AUTOPLAY_GAP)}px)"
        role="list"
        bind:scroller
      >
        {#snippet children(index)}
          {@const item = items[index]}
          {#if item}
            <!-- A plain list item holding two sibling controls (play, remove):
                 no interactive element nests inside another. Dragging is
                 delegated from the scroller (pointerdown). -->
            <div
              class="q-row row-wash"
              class:current={index === currentIndex}
              class:played={currentIndex !== null && index < currentIndex}
              class:placeholder={drag?.from === index}
              role="listitem"
              data-q-index={index}
              aria-posinset={index + 1}
              aria-setsize={items.length}
              aria-current={index === currentIndex ? "true" : undefined}
              style:transform={rowTransform(index)}
            >
              {#if index === autoplayFrom && !drag && dropSlot === null}
                <!-- Rides with the row into the gap gapBefore leaves above it. -->
                <span class="q-autoplay" aria-hidden="true" style:height="{AUTOPLAY_GAP}px">Autoplay</span>
              {/if}
              <span class="grip" aria-hidden="true"><Icon name="grip-vertical" size={14} /></span>
              <button
                type="button"
                class="row-main"
                tabindex={index === tabIndex ? 0 : -1}
                aria-label={`${item.title} — ${item.artist}${item.autoplay ? " (Autoplay)" : ""}`}
                aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown Delete"
                title="Play"
                use:menuFor={() => {
                  const to = nextSlot(index);
                  return queueMenuItems({
                    index,
                    playNow: () => jump(index),
                    playNext: to === null ? undefined : () => reorder(index, to),
                    copyTitle: () => void copyText(item.title),
                    remove: () => player.remove(index),
                    ...(go ? goToTargets(item, go, api.getTrack) : {}),
                  });
                }}
                onfocus={() => (focusIndex = index)}
                onkeydown={(event) => onRowKey(event, index)}
                onclick={() => onRowClick(index)}
              >
                <!-- The playing row's cover + text slide right to reveal the
                     spinning disc beside the cover. -->
                {#if index === currentIndex}
                  <span
                    class="np-disc"
                    aria-hidden="true"
                    in:discIn={{ reduced: appearance.reducedMotion }}
                    out:fade={{ duration: appearance.reducedMotion ? 0 : 140 }}
                  ><NowPlayingMark playing={player.state.playing} size={22} /></span>
                {/if}
                <span class="slide">{@render rowBody(item, true)}</span>
              </button>
              <span class="dur tnum">{formatDuration(item.duration_secs)}</span>
              <button
                class="remove"
                tabindex="-1"
                onclick={() => player.remove(index)}
                aria-label={`Remove ${item.title}`}
                title="Remove"
              ><Icon name="x" size={14} /></button>
            </div>
          {/if}
        {/snippet}
      </VirtualList>
    </div>
  {/if}

  {#if drag && press && items[drag.from]}
    <!-- The lifted row follows the pointer outside the virtual list, so it
         survives its own row scrolling out of the rendered window. -->
    <div
      class="q-ghost"
      aria-hidden="true"
      style:top="{drag.y - press.grab - press.railTop}px"
      style:left="{press.left}px"
      style:width="{press.width}px"
    >
      <span class="ghost-main">{@render rowBody(items[drag.from], false)}</span>
      <span class="dur tnum">{formatDuration(items[drag.from].duration_secs)}</span>
    </div>
  {/if}

  {#if confirmingClear}
    <ConfirmDialog
      title="Clear queue"
      message="Remove all tracks from the queue?"
      onconfirm={() => {
        confirmingClear = false;
        playSource.set(null);
        player.clear();
      }}
      oncancel={() => (confirmingClear = false)}
    />
  {/if}
</div>

<style>
  .queue-rail {
    --q-thumb: calc(var(--q-row) - 14px);
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    height: 100%;
    min-height: 0;
  }

  .q-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px 12px 10px 16px;
  }
  .q-title {
    margin: 0;
    font-size: 22px;
  }
  .q-meta {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-muted);
    font-size: 12.5px;
    padding-top: 4px;
  }
  .q-tools {
    display: flex;
    gap: 2px;
    margin-left: auto;
  }
  .q-tool {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    color: var(--text-muted);
  }
  .q-tool:hover:not(:disabled) {
    background: var(--surface-3);
    color: var(--text);
  }

  /* Flex sizing (not a percentage max-height) bounds the virtual list to the
     space left under the header, whatever the host container is. */
  .q-list {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
  }
  /* Where an app drag would insert (drag-to-playlist spec DD-D8). */
  .q-drop-line {
    position: absolute;
    z-index: 2;
    left: 12px;
    right: 12px;
    height: 2px;
    margin-top: -1px;
    border-radius: 1px;
    background: var(--accent);
    pointer-events: none;
  }
  /* An empty queue under a drag: the whole rail is the target. */
  .drop-over .q-empty {
    color: var(--accent);
  }
  .q-list > :global(.scroll) {
    flex: 1 1 0;
    min-height: 0;
  }

  .q-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 48px 24px;
    text-align: center;
    color: var(--text-muted);
  }
  .q-empty p {
    margin: 0;
  }
  .q-empty p:first-of-type {
    margin-top: 6px;
    color: var(--text);
    font-weight: 600;
  }
  .q-empty p.muted {
    font-size: 12.5px;
  }

  .q-row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    height: var(--q-row);
    /* Full-bleed: hover and the current-track tint (`--row-wash`, app.css)
       span the rail and fade in from its edges. */
    padding: 0 14px 0 16px;
    user-select: none;
    transition:
      transform var(--dur-state) var(--ease-in-out),
      --row-wash var(--dur-micro);
  }
  .settling .q-row {
    transition: none;
  }
  .q-row:hover {
    --row-wash: var(--surface-2);
  }
  .q-row.current {
    --row-wash: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  .q-row.current:hover {
    --row-wash: color-mix(in srgb, var(--accent) 24%, transparent);
  }
  .q-row.played .thumb,
  .q-row.played .text,
  .q-row.played .dur {
    opacity: 0.5;
  }

  /* Eyebrow over the first upcoming Autoplay suggestion, on a hairline. */
  .q-autoplay {
    position: absolute;
    left: 16px;
    right: 14px;
    bottom: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    pointer-events: none;
  }
  .q-autoplay::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--line);
  }

  /* The dragged row's own slot travels to the drop position as an outlined
     gap; its content rides in the ghost. */
  .q-row.placeholder {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    outline: 1.5px dashed color-mix(in srgb, var(--accent) 60%, transparent);
    outline-offset: -3px;
  }
  .q-row.placeholder > * {
    visibility: hidden;
  }
  /* No hover washes while dragging: the gap is the only highlight. */
  .dragging .q-row:not(.placeholder):not(.current):hover {
    --row-wash: transparent;
  }

  .grip {
    position: absolute;
    left: 1px;
    top: 0;
    bottom: 0;
    display: grid;
    place-items: center;
    width: 14px;
    color: var(--text-muted);
    opacity: 0;
    cursor: grab;
    transition: opacity var(--dur-micro);
  }
  .q-row:hover .grip {
    opacity: 0.7;
  }
  .dragging .grip {
    opacity: 0;
  }

  .row-main,
  .ghost-main,
  .slide {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 12px;
    min-width: 0;
    height: 100%;
    padding: 0;
    text-align: left;
  }
  .row-main {
    position: relative;
  }
  /* Current row: cover + text glide right (transform only) to uncover the
     disc. The matching right padding is applied at once so the text's
     ellipsis edge stays put instead of sliding under the duration. */
  .slide {
    --disc-shift: 34px;
    transition: transform var(--dur-view) var(--ease-enter);
  }
  .q-row.current .slide {
    padding-right: var(--disc-shift);
    transform: translateX(var(--disc-shift));
  }
  .np-disc {
    position: absolute;
    left: 0;
    top: 50%;
    display: grid;
    margin-top: -11px;
  }
  .thumb {
    position: relative;
    flex: 0 0 auto;
    width: var(--q-thumb);
    height: var(--q-thumb);
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .title,
  .artist {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    font-weight: 500;
  }
  .artist {
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .artist :global(.name-link:hover) {
    color: var(--text);
  }
  .q-row.current .title {
    color: var(--accent);
  }

  /* Duration at rest; the remove button takes its place on hover/focus. */
  .dur,
  .remove {
    flex: 0 0 auto;
    width: 36px;
    text-align: right;
  }
  .dur {
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .remove {
    display: none;
    place-items: center;
    height: 28px;
    border-radius: 50%;
    color: var(--text-muted);
  }
  .remove:hover {
    color: var(--text);
    background: var(--surface-3);
  }
  .q-row:hover .remove,
  .q-row:focus-within .remove {
    display: grid;
  }
  .q-row:hover .dur,
  .q-row:focus-within .dur {
    display: none;
  }
  .dragging .q-row .remove {
    display: none;
  }
  .dragging .q-row .dur {
    display: block;
  }

  .q-ghost {
    position: absolute;
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 6px;
    height: var(--q-row);
    padding: 0 14px 0 16px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background:
      linear-gradient(var(--surface-float), var(--surface-float)) padding-box,
      var(--edge-lit) border-box;
    /* No backdrop-filter: the ghost lives inside the queue's glass, and nested
       glass is dropped by Chromium (and barred by the motion rules); the
       opaque float veil carries it. */
    box-shadow: 0 14px 32px rgb(0 0 0 / 0.45);
    transform: scale(1.02);
    pointer-events: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .q-row {
      transition: --row-wash var(--dur-micro);
    }
    .q-ghost {
      transform: none;
    }
  }
  :global(body.queue-dragging),
  :global(body.queue-dragging *) {
    cursor: grabbing !important;
  }
</style>
