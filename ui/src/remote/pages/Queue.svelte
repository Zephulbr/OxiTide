<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import { formatDuration } from "../../lib/format";
  import Art from "../components/Art.svelte";
  import { useRemote } from "../context";
  import { art } from "../convert";
  import { dropIndex, visibleRange } from "../queueDrag";

  const { store, api, actions } = useRemote();

  const ROW = 64;
  let scrollY = $state(0);
  let innerHeight = $state(800);
  let list = $state<HTMLOListElement | null>(null);
  let listTop = $state(0);
  let drag = $state<{ from: number; startY: number; dy: number } | null>(null);

  const total = $derived(store.state?.queueLen ?? 0);
  const currentIndex = $derived(store.state?.currentIndex ?? null);

  $effect(() => {
    void total;
    if (list) listTop = list.getBoundingClientRect().top + window.scrollY;
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
    if (currentIndex === null) return;
    window.scrollTo({ top: listTop + currentIndex * ROW - innerHeight / 3, behavior: "smooth" });
  }

  function confirmClear() {
    actions.sheet = {
      title: "Clear the queue?",
      subtitle: "Playback stops on the PC.",
      items: [{ label: "Clear queue", icon: "x", run: () => run(api.queueClear()) }],
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
    <h1>Queue</h1>
    {#if currentIndex !== null}
      <button class="icon-btn" aria-label="Show current track" onclick={jumpToCurrent}>
        <Icon name="circle-dot" size={20} />
      </button>
    {/if}
    <button class="pill" disabled={total === 0} onclick={confirmClear}>Clear</button>
  </header>
  {#if total === 0}
    <p class="muted">The queue is empty. Play something from Home, Search, or your Library.</p>
  {:else}
    <p class="muted count">{total === 1 ? "1 track" : `${total} tracks`}</p>
    <ol class="list" bind:this={list} style:height={`${total * ROW}px`}>
      {#each rows as index (index)}
        {@const item = store.queueItem(index)}
        {@const dragging = drag?.from === index}
        <li
          class="row"
          class:current={index === currentIndex}
          class:dragging
          style:top={`${index * ROW}px`}
          style:transform={dragging && drag ? `translateY(${drag.dy}px)` : undefined}
        >
          <button
            class="handle"
            aria-label={`Reorder ${item?.title ?? `track ${index + 1}`}`}
            onpointerdown={(e) => startDrag(e, index)}
            onpointermove={moveDrag}
            onpointerup={endDrag}
            onpointercancel={() => (drag = null)}
          >
            <Icon name="grip-vertical" size={18} />
          </button>
          {#if item}
            <button class="main" onclick={() => run(api.queueJump(index))} aria-label={`Play ${item.title}`}>
              <Art src={art(item.cover, 80)} size={44} />
              <span class="text">
                <span class="title ellipsis">{item.title}</span>
                <span class="muted ellipsis">{item.artist}</span>
              </span>
              <span class="muted time">{formatDuration(item.duration_secs)}</span>
            </button>
            <button class="icon-btn" aria-label={`Remove ${item.title}`} onclick={() => run(api.queueRemove(index))}>
              <Icon name="x" size={18} />
            </button>
          {:else}
            <span class="main placeholder muted">Loading…</span>
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
  .current .title {
    color: var(--accent);
  }
  .time {
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .placeholder {
    padding-left: 56px;
  }
</style>
