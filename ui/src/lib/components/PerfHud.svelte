<script lang="ts">
  import { perf } from "../perf.svelte";
  import { formatHudLines } from "../perf";

  function onKeydown(event: KeyboardEvent) {
    if (!perf.active) return;
    if (event.ctrlKey && event.shiftKey && event.key.toLowerCase() === "h") {
      event.preventDefault();
      perf.toggleHud();
    }
  }

  const lines = $derived(
    perf.snapshot
      ? formatHudLines(perf.snapshot, perf.frames.summary(), perf.longTasks.summary())
      : [],
  );
</script>

<svelte:window onkeydown={onKeydown} />

{#if perf.active && perf.hudVisible}
  <aside class="perf-hud" aria-label="Performance HUD">
    {#each lines as line}<div>{line}</div>{/each}
  </aside>
{/if}

<style>
  .perf-hud {
    position: fixed;
    right: 12px;
    bottom: 12px;
    z-index: 9999;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgb(0 0 0 / 0.78);
    color: #fff;
    font: 11px/1.5 monospace;
    pointer-events: none;
    white-space: pre;
  }
</style>
