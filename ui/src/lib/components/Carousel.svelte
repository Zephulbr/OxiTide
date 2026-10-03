<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    controls,
    children,
  }: { title: string; controls?: Snippet; children: Snippet } = $props();

  let el = $state<HTMLElement | null>(null);
  let canLeft = $state(false);
  let canRight = $state(false);

  function sync() {
    if (!el) return;
    canLeft = el.scrollLeft > 4;
    canRight = el.scrollLeft + el.clientWidth < el.scrollWidth - 4;
  }

  $effect(() => {
    if (!el) return;
    sync();
    const observer = new ResizeObserver(sync);
    observer.observe(el);
    return () => observer.disconnect();
  });

  function step(direction: -1 | 1) {
    if (!el) return;
    const card = el.querySelector<HTMLElement>(".media");
    const amount = card ? card.offsetWidth + 16 : el.clientWidth * 0.8;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    el.scrollBy({ left: direction * amount, behavior: reduced ? "auto" : "smooth" });
  }
</script>

<div class="head">
  <h3>{title}</h3>
  <div class="controls">
    <button
      type="button"
      class="control"
      onclick={() => step(-1)}
      disabled={!canLeft}
      aria-label="Scroll left"
      title="Scroll left"
    ><Icon name="chevron-left" size={16} /></button>
    <button
      type="button"
      class="control"
      onclick={() => step(1)}
      disabled={!canRight}
      aria-label="Scroll right"
      title="Scroll right"
    ><Icon name="chevron-right" size={16} /></button>
    {@render controls?.()}
  </div>
</div>
<div class="row" bind:this={el} onscroll={sync} role="region" aria-label={title}>
  {@render children()}
</div>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .head h3 {
    flex: 1;
    min-width: 0;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .control {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 1px solid var(--line);
    border-radius: 50%;
    color: var(--text-muted);
  }
  .control:hover:not(:disabled) {
    color: var(--text);
  }
  .control:disabled {
    opacity: 0.4;
  }
  .row {
    display: flex;
    gap: 20px;
    overflow-x: auto;
    scroll-snap-type: x proximity;
    padding: 2px 2px 8px;
    scrollbar-width: none;
  }
  .row::-webkit-scrollbar {
    display: none;
  }
</style>
