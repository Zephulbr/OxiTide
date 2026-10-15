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
    bottomPadding = "0px",
    role = undefined,
    scroller = $bindable(null),
    children,
  }: {
    total: number;
    rowHeight: number;
    maxHeight?: string;
    overscan?: number;
    drag?: DragParams;
    bottomPadding?: string;
    /// ARIA role for the scroller; rows then carry the matching item role.
    role?: "list";
    /// The scrolling element, for hosts that drive scrollTop themselves.
    scroller?: HTMLElement | null;
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
  {role}
  style="max-height:{maxHeight};padding-bottom:{bottomPadding}"
  bind:this={scroller}
  bind:clientHeight={viewport}
  onscroll={onScroll}
  use:trackDrag={drag ?? NO_DRAG}
>
  <div style="height:{range.padTop}px" aria-hidden="true"></div>
  {#each indexes as index (index)}
    {@render children(index)}
  {/each}
  <div style="height:{range.padBottom}px" aria-hidden="true"></div>
</div>

<style>
  .scroll {
    overflow: auto;
  }
</style>
