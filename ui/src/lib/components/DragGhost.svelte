<script lang="ts">
  import { dragDrop } from "../dragDrop.svelte";
  import { payloadCount, payloadCovers, payloadLabel } from "../dragPayload";
  import Cover from "./Cover.svelte";

  /// The pill under the pointer while dragging (drag-to-playlist spec
  /// DD-D11). The controller moves it through its style each frame; this
  /// component re-renders only when the payload or the verdict changes.
  const payload = $derived(dragDrop.active);
  const verdict = $derived(dragDrop.verdict);
  const covers = $derived(payload ? payloadCovers(payload) : []);
  const count = $derived(payload ? payloadCount(payload) : null);
  const label = $derived(payload ? payloadLabel(payload) : "");
  const hint = $derived(verdict ? (verdict.ok ? verdict.hint : verdict.reason) : "");

  function ghost(node: HTMLElement) {
    dragDrop.bindGhost(node);
    return { destroy: () => dragDrop.bindGhost(null) };
  }
</script>

{#if payload}
  <div class="ghost" class:refused={verdict !== null && !verdict.ok} aria-hidden="true" use:ghost>
    <span class="thumbs" class:stacked={covers.length > 1}>
      {#each covers as cover, i (i)}
        <span class="thumb"><Cover uuid={cover} size={80} radius={4} /></span>
      {/each}
      {#if count !== null && count > 1}<span class="count tnum">{count}</span>{/if}
    </span>
    <span class="text">
      <span class="label">{label}</span>
      {#if hint}<span class="hint">{hint}</span>{/if}
    </span>
  </div>
{/if}

<style>
  .ghost {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 280px;
    /* Offset from the pointer so the hit test and the target stay visible. */
    margin: 12px 0 0 14px;
    padding: 5px 12px 5px 5px;
    /* The context menu's floating glass, so it reads over any cover. */
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--accent) 8%, var(--surface-float));
    backdrop-filter: blur(var(--glass-blur));
    color: var(--text);
    box-shadow: 0 10px 28px rgb(0 0 0 / 0.4);
    pointer-events: none;
    will-change: transform;
    animation: ghost-in var(--dur-micro) var(--ease-enter);
  }
  .thumbs {
    position: relative;
    flex: none;
    width: 34px;
    height: 34px;
  }
  .thumb {
    position: absolute;
    inset: 0;
    border-radius: 4px;
    overflow: hidden;
  }
  /* Several tracks: a second cover peeks out behind the first. */
  .stacked .thumb:nth-child(2) {
    inset: -3px -3px 3px 3px;
    z-index: -1;
    opacity: 0.7;
  }
  .count {
    position: absolute;
    right: -7px;
    bottom: -6px;
    min-width: 18px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 11px;
    font-weight: 700;
    line-height: 18px;
    text-align: center;
  }
  .text {
    display: grid;
    min-width: 0;
    line-height: 1.25;
  }
  .label,
  .hint {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .label {
    font-size: 13px;
    font-weight: 600;
  }
  .hint {
    font-size: 12px;
    color: var(--accent);
  }
  .refused .hint {
    color: var(--text-muted);
  }
  @keyframes ghost-in {
    from {
      opacity: 0;
      scale: 0.92;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ghost {
      animation: none;
    }
  }
</style>
