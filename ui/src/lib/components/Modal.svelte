<script lang="ts">
  import type { Snippet } from "svelte";
  let {
    title,
    onclose,
    children,
  }: { title: string; onclose: () => void; children: Snippet } = $props();

  let panel = $state<HTMLDivElement | null>(null);

  $effect(() => {
    const node = panel;
    if (!node) return;
    const previous = document.activeElement as HTMLElement | null;
    const first = node.querySelector<HTMLElement>(
      'input, select, textarea, button, [href], [tabindex]:not([tabindex="-1"])',
    );
    first?.focus();
    return () => {
      if (previous?.isConnected) previous.focus();
    };
  });
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === "Escape") onclose();
  }}
/>

<div class="overlay">
  <div class="panel" role="dialog" aria-modal="true" aria-label={title} bind:this={panel}>
    <header>
      <strong>{title}</strong>
      <button onclick={onclose} aria-label="Close">✕</button>
    </header>
    {@render children()}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgb(0 0 0 / 0.5);
  }
  .panel {
    max-width: min(560px, 90vw);
    max-height: 80vh;
    overflow: auto;
    padding: 16px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--panel);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
</style>
