<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import { formatDuration } from "../../lib/format";
  import Art from "../components/Art.svelte";
  import { useRemote } from "../context";
  import { art } from "../convert";
  import { motion } from "../motion";
  import { dropIndex, rowOnScreen, scrollTopFor, visibleRange } from "../queueDrag";
  import { t } from "../i18n.svelte";
  import { untrack } from "svelte";

  const { store, api, actions } = useRemote();

  const ROW = 64;
  let scrollY = $state(0);
  let innerHeight = $state(800);
  let list = $state<HTMLOListElement | null>(null);
  let listTop = $state(0);
  let drag = $state<{ from: number; startY: number; dy: number } | null>(null);

  const total = $derived(store.state?.queueLen ?? 0);
  const currentIndex = $derived(store.state?.currentIndex ?? null);
  const currentId = $derived(store.state?.current?.trackId ?? null);
  const playing = $derived(store.state?.playing ?? false);

  /// The list's top in page coordinates. Summed offsets, not the bounding
  /// box: the page flies in with a transform when it opens.
  function pageTop(el: HTMLElement): number {
    let top = 0;
    for (let node: HTMLElement | null = el; node; node = node.offsetParent as HTMLElement | null) {
      top += node.offsetTop;
    }
    return top;
  }

  $effect(() => {
    void total;
    if (list) listTop = pageTop(list);
  });

  /// Height of the dock (mini bar + tab bar) over the bottom of the window.
  function dockHeight(): number {
    const style = getComputedStyle(document.documentElement);
    return (
      (parseFloat(style.getPropertyValue("--remote-tabbar")) || 0) +
      (parseFloat(style.getPropertyValue("--remote-minibar")) || 0)
    );
  }

  function scrollToRow(index: number, smooth: boolean) {
    const top = scrollTopFor(index, ROW, listTop);
    window.scrollTo({ top, behavior: smooth && motion(1) > 0 ? "smooth" : "auto" });
  }

  // Open on the current track (like the desktop queue rail); afterwards
  // follow it to the next track only while its row was on screen, so a
  // listener who scrolled away to browse is left where they are.
  let followed: { index: number; id: number | null } | null = null;
  $effect(() => {
    const index = currentIndex;
    const id = currentId;
    const top = listTop;
    if (!list || index === null || top === 0) return;
    untrack(() => {
      if (followed === null) scrollToRow(index, false);
      else if (
        (index !== followed.index || id !== followed.id) &&
        drag === null &&
        rowOnScreen(followed.index, ROW, top, window.scrollY, window.innerHeight, dockHeight())
      ) {
        scrollToRow(index, true);
      }
      followed = { index, id };
    });
  });

  const range = $derived(visibleRange(scrollY - listTop, innerHeight, ROW, total));
  const rows = $derived.by(() => {
    void store.queueVersion;
    const out: number[] = [];
    for (let i = range.from; i < range.to; i++) out.push(i);
    if (drag && (drag.from < range.from || drag.from >= range.to)) out.push(drag.from);
    return out;
  });

  $effect(() => {
    void store.queueVersion;
    void store.ensureQueue(range.from, range.to);
  });

  function run(task: Promise<unknown>) {
    void task.catch((e) => actions.fail(e));
  }

  function jumpToCurrent() {
    if (currentIndex !== null) scrollToRow(currentIndex, true);
  }

  function confirmClear() {
    actions.sheet = {
      title: t("queue.clearTitle"),
      subtitle: t("queue.clearSubtitle"),
      items: [{ label: t("queue.clear"), icon: "x", run: () => run(api.queueClear()) }],
    };
  }

  function startDrag(event: PointerEvent, index: number) {
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    drag = { from: index, startY: event.clientY, dy: 0 };
  }

  function moveDrag(event: PointerEvent) {
    if (drag) drag.dy = event.clientY - drag.startY;
  }

  function endDrag() {
    if (!drag) return;
    const to = dropIndex(drag.from, drag.dy, ROW, total);
    const from = drag.from;
    drag = null;
    if (to !== from) run(api.queueReorder(from, to));
  }
</script>

<svelte:window bind:scrollY bind:innerHeight />

<div class="page">
  <header class="page-head">
    <h1>{t("tab.queue")}</h1>
    {#if currentIndex !== null}
      <button class="icon-btn" aria-label={t("queue.showCurrent")} onclick={jumpToCurrent}>
        <Icon name="circle-dot" size={20} />
      </button>
    {/if}
    <button class="pill" disabled={total === 0} onclick={confirmClear}>{t("queue.clearShort")}</button>
  </header>
  {#if total === 0}
    <p class="muted">{t("queue.empty")}</p>
  {:else}
    <p class="muted count">{t("common.trackCount", { count: total })}</p>
    <ol class="list" bind:this={list} style:height={`${total * ROW}px`}>
      {#each rows as index (index)}
        {@const item = store.queueItem(index)}
        {@const dragging = drag?.from === index}
        <li
          class="row"
          class:current={index === currentIndex}
          class:played={currentIndex !== null && index < currentIndex}
          aria-current={index === currentIndex ? "true" : undefined}
          class:dragging
          style:top={`${index * ROW}px`}
          style:transform={dragging && drag ? `translateY(${drag.dy}px)` : undefined}
        >
          <button
            class="handle"
            aria-label={item ? t("queue.reorder", { title: item.title }) : t("queue.reorderN", { n: index + 1 })}
            onpointerdown={(e) => startDrag(e, index)}
            onpointermove={moveDrag}
            onpointerup={endDrag}
            onpointercancel={() => (drag = null)}
          >
            <Icon name="grip-vertical" size={18} />
          </button>
          {#if item}
            <button class="main" onclick={() => run(api.queueJump(index))} aria-label={t("common.playNamed", { title: item.title })}>
              <span class="cover">
                <Art src={art(item.cover, 80)} size={44} />
                {#if index === currentIndex}
                  <!-- The desktop's now-playing disc: turns while the PC plays. -->
                  <span class="now" class:spinning={playing} aria-hidden="true"><Icon name="disc-3" size={22} /></span>
                {/if}
              </span>
              <span class="text">
                <span class="title ellipsis">{item.title}</span>
                <!-- Paged rows can't find the first suggestion, so each says so. -->
                <span class="muted ellipsis">{#if item.autoplay}<span class="autoplay">{t("queue.autoplay")}</span> · {/if}{item.artist}</span>
              </span>
              <span class="muted time">{formatDuration(item.duration_secs)}</span>
            </button>
            <button class="icon-btn" aria-label={t("queue.remove", { title: item.title })} onclick={() => run(api.queueRemove(index))}>
              <Icon name="x" size={18} />
            </button>
          {:else}
            <span class="main placeholder muted">{t("common.loading")}</span>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  .count {
    margin: -8px 0 8px;
  }
  .list {
    position: relative;
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 64px;
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .row.dragging {
    z-index: 2;
    border-radius: var(--radius-control);
    background: var(--surface-3);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
  }
  .handle {
    display: grid;
    place-items: center;
    width: 36px;
    height: 100%;
    color: var(--text-muted);
    touch-action: none;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    text-align: left;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    font-size: 13px;
  }
  .title {
    font-size: 15px;
    font-weight: 600;
  }
  /* The playing row: a wash and an accent bar (a dynamic accent can be
     near-grey on a black-and-white cover, so colour alone is not enough). */
  .row.current {
    border-radius: var(--radius-control);
    background: var(--surface-2);
  }
  .row.current::before {
    content: "";
    position: absolute;
    left: 0;
    top: 12px;
    bottom: 12px;
    width: 3px;
    border-radius: 2px;
    background: var(--accent);
  }
  .current .title {
    color: var(--accent);
  }
  /* Played rows fade back, as in the desktop queue. */
  .played .cover,
  .played .text,
  .played .time {
    opacity: 0.5;
  }
  .cover {
    position: relative;
    flex: none;
    display: grid;
  }
  .now {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    border-radius: var(--radius-card-sharp);
    background: color-mix(in srgb, var(--bg) 55%, transparent);
    color: var(--accent);
  }
  .now :global(svg) {
    animation: turn 2s steps(60) infinite;
    animation-play-state: paused;
  }
  .now.spinning :global(svg) {
    animation-play-state: running;
  }
  @keyframes turn {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .now :global(svg) {
      animation: none;
    }
  }
  .autoplay {
    font-weight: 600;
  }
  .time {
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .placeholder {
    padding-left: 56px;
  }
</style>
