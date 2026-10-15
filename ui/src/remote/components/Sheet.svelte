<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import { useRemote } from "../context";

  const { actions } = useRemote();
  let panel = $state<HTMLDivElement | null>(null);

  $effect(() => {
    if (actions.sheet) panel?.querySelector<HTMLButtonElement>("button")?.focus();
  });
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === "Escape") actions.sheet = null;
  }}
/>

{#if actions.sheet}
  {@const sheet = actions.sheet}
  <div class="scrim" role="presentation" onclick={() => (actions.sheet = null)}></div>
  <div class="sheet" role="dialog" aria-modal="true" aria-label={sheet.title} bind:this={panel}>
    <header>
      <strong class="ellipsis">{sheet.title}</strong>
      {#if sheet.subtitle}<span class="muted ellipsis">{sheet.subtitle}</span>{/if}
    </header>
    <ul>
      {#each sheet.items as item, i (`${item.label}-${i}`)}
        <li>
          <button onclick={() => actions.runSheet(item)}>
            <Icon name={item.icon} size={20} />
            <span class="ellipsis">{item.label}</span>
          </button>
        </li>
      {/each}
    </ul>
    <button class="cancel" onclick={() => (actions.sheet = null)}>Cancel</button>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: color-mix(in srgb, var(--bg) 60%, transparent);
  }
  .sheet {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 41;
    max-height: 75vh;
    overflow-y: auto;
    padding: 12px var(--remote-gutter) calc(12px + env(safe-area-inset-bottom));
    border-radius: var(--radius-sheet) var(--radius-sheet) 0 0;
    border-top: 1px solid var(--line);
    background: var(--surface-float);
    backdrop-filter: blur(var(--overlay-blur));
    -webkit-backdrop-filter: blur(var(--overlay-blur));
    animation: rise var(--dur-view) var(--ease-enter);
  }
  @keyframes rise {
    from {
      transform: translateY(24px);
      opacity: 0;
    }
  }
  header {
    display: flex;
    flex-direction: column;
    padding: 4px 4px 10px;
    border-bottom: 1px solid var(--line);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px 0;
  }
  li button {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    min-height: 48px;
    padding: 0 4px;
    text-align: left;
  }
  .cancel {
    width: 100%;
    min-height: 48px;
    border-radius: var(--radius-control);
    background: var(--surface-2);
    font-weight: 600;
  }
</style>
