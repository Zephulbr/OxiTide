<script lang="ts">
  import type { Snippet } from "svelte";
  import { trackDrag, type DragParams } from "../drag";
  import { windowRange } from "../virtual";

  let {
    total,
    rowHeight,
    maxHeight = "none",
    overscan = 8,
    drag,
    children,
  }: {
    total: number;
    rowHeight: number;
    maxHeight?: string;
    overscan?: number;
    drag?: DragParams;
    children: Snippet<[number]>;
  } = $props();

  const NO_DRAG: DragParams = {
    enabled: () => false,
    count: () => 0,
    rowHeight: () => 1,
    onChange: () => {},
    onDrop: () => {},
  };

  let scrollTop = $state(0);
  let viewport = $state(0);
  const range = $derived(windowRange(total, scrollTop, viewport, rowHeight, overscan));
  const indexes = $derived(
    Array.from({ length: Math.max(0, range.last - range.first) }, (_, i) => range.first + i),
  );

  function onScroll(event: Event) {
    scrollTop = (event.currentTarget as HTMLElement).scrollTop;
  }
</script>

<div
  class="scroll"
  style="max-height:{maxHeight}"
  bind:clientHeight={viewport}
  onscroll={onScroll}
  use:trackDrag={drag ?? NO_DRAG}
>
  <div style="height:{range.padTop}px"></div>
  {#each indexes as index (index)}
    {@render children(index)}
  {/each}
  <div style="height:{range.padBottom}px"></div>
</div>

<style>
  .scroll {
    overflow: auto;
  }
</style>
