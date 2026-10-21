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
    // Step by one card plus the row's real gap so repeated clicks stay aligned.
    const gap = parseFloat(getComputedStyle(el).columnGap) || 0;
    const amount = card ? card.offsetWidth + gap : el.clientWidth * 0.8;
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
<div
  class="row pane"
  class:fade-left={canLeft}
  class:fade-right={canRight}
  bind:this={el}
  onscroll={sync}
  role="region"
  aria-label={title}
>
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
    --gap: 20px;
    display: flex;
    gap: var(--gap);
    overflow-x: auto;
    container-type: inline-size;
    scroll-snap-type: x proximity;
    padding: 2px 2px 8px;
    scrollbar-width: none;
    --fade: 72px;
  }
  .row::-webkit-scrollbar {
    display: none;
  }
  /* Cards size to the row so a part of the next one always peeks past the
     edge (and fades): --per whole cards plus 0.4 of one. The count steps
     with the row's width (container queries), keeping a card near its
     nominal size: 190-220px for the regular card, 136-152px for Home's
     compact one. The width selector outranks MediaCard's fixed widths. */
  .row > :global(.media) {
    --per: 1;
  }
  .row > :global(.media.carousel),
  .row > :global(.media.mini) {
    width: max(120px, calc((100cqi - var(--per) * var(--gap)) / (var(--per) + 0.4)));
  }
  @container (min-width: 496px) { .row > :global(.carousel) { --per: 2; } }
  @container (min-width: 706px) { .row > :global(.carousel) { --per: 3; } }
  @container (min-width: 917px) { .row > :global(.carousel) { --per: 4; } }
  @container (min-width: 1126px) { .row > :global(.carousel) { --per: 5; } }
  @container (min-width: 1336px) { .row > :global(.carousel) { --per: 6; } }
  @container (min-width: 1546px) { .row > :global(.carousel) { --per: 7; } }
  @container (min-width: 1756px) { .row > :global(.carousel) { --per: 8; } }
  @container (min-width: 1966px) { .row > :global(.carousel) { --per: 9; } }
  @container (min-width: 2176px) { .row > :global(.carousel) { --per: 10; } }
  @container (min-width: 2386px) { .row > :global(.carousel) { --per: 11; } }
  @container (min-width: 2596px) { .row > :global(.carousel) { --per: 12; } }
  @container (min-width: 367px) { .row > :global(.mini) { --per: 2; } }
  @container (min-width: 523px) { .row > :global(.mini) { --per: 3; } }
  @container (min-width: 679px) { .row > :global(.mini) { --per: 4; } }
  @container (min-width: 835px) { .row > :global(.mini) { --per: 5; } }
  @container (min-width: 991px) { .row > :global(.mini) { --per: 6; } }
  @container (min-width: 1147px) { .row > :global(.mini) { --per: 7; } }
  @container (min-width: 1303px) { .row > :global(.mini) { --per: 8; } }
  @container (min-width: 1459px) { .row > :global(.mini) { --per: 9; } }
  @container (min-width: 1615px) { .row > :global(.mini) { --per: 10; } }
  @container (min-width: 1771px) { .row > :global(.mini) { --per: 11; } }
  @container (min-width: 1927px) { .row > :global(.mini) { --per: 12; } }
  @container (min-width: 2083px) { .row > :global(.mini) { --per: 13; } }
  @container (min-width: 2239px) { .row > :global(.mini) { --per: 14; } }
  @container (min-width: 2395px) { .row > :global(.mini) { --per: 15; } }
  @container (min-width: 2551px) { .row > :global(.mini) { --per: 16; } }
  /* A side with more to scroll fades out (72px), so a card cut at the edge
     reads as "more this way" rather than a clipped render. */
  .row.fade-right {
    mask-image: linear-gradient(to right, #000 calc(100% - var(--fade)), transparent);
  }
  .row.fade-left {
    mask-image: linear-gradient(to right, transparent, #000 var(--fade));
  }
  .row.fade-left.fade-right {
    mask-image: linear-gradient(
      to right,
      transparent,
      #000 var(--fade),
      #000 calc(100% - var(--fade)),
      transparent
    );
  }
</style>
