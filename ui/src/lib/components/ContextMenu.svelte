<script lang="ts">
  import type { Menu, MenuItem } from "../menu.svelte";

  let { menu }: { menu: Menu } = $props();

  let panel = $state<HTMLDivElement | null>(null);
  let left = $state(0);
  let top = $state(0);
  let active = $state(-1);

  function enabledIndexes(items: MenuItem[]): number[] {
    return items.map((item, index) => (item.disabled ? -1 : index)).filter((i) => i >= 0);
  }

  function clampToViewport() {
    const node = panel;
    const state = menu.open;
    if (!node || !state) return;
    const rect = node.getBoundingClientRect();
    left = Math.max(4, Math.min(state.x, window.innerWidth - rect.width - 4));
    top = Math.max(4, Math.min(state.y, window.innerHeight - rect.height - 4));
  }

  function moveActive(delta: -1 | 1) {
    const state = menu.open;
    if (!state) return;
    const enabled = enabledIndexes(state.items);
    if (enabled.length === 0) return;
    const current = enabled.indexOf(active);
    const next = current === -1 ? 0 : (current + delta + enabled.length) % enabled.length;
    active = enabled[next];
  }

  function run(item: MenuItem) {
    if (item.disabled) return;
    menu.close();
    item.run();
  }

  // Cursor reset plus a viewport clamp after the panel has measurable size.
  $effect(() => {
    const state = menu.open;
    if (!state) return;
    const enabled = enabledIndexes(state.items);
    active = enabled.length > 0 ? enabled[0] : -1;
    clampToViewport();
  });

  // Focus the panel so arrows/Enter/Escape stay inside the menu.
  $effect(() => {
    if (menu.open && panel) panel.focus();
  });

  // Close when anything scrolls, including nested scrollers.
  $effect(() => {
    if (!menu.open) return;
    const close = () => menu.close();
    window.addEventListener("scroll", close, true);
    return () => window.removeEventListener("scroll", close, true);
  });

  function onKeydown(event: KeyboardEvent) {
    const state = menu.open;
    if (!state) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      moveActive(event.key === "ArrowDown" ? 1 : -1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      const item = active >= 0 ? state.items[active] : undefined;
      if (item) run(item);
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      menu.close();
    }
  }

  function onWindowMouseDown(event: MouseEvent) {
    if (!menu.open) return;
    if (panel && event.target instanceof Node && panel.contains(event.target)) return;
    menu.close();
  }
</script>

<svelte:window onmousedown={onWindowMouseDown} onresize={() => menu.close()} />

{#if menu.open}
  <div
    bind:this={panel}
    class="menu"
    role="menu"
    tabindex="-1"
    style="left:{left}px;top:{top}px"
    onkeydown={onKeydown}
  >
    {#each menu.open.items as item, i (item.id)}
      <button
        type="button"
        role="menuitem"
        class:active={active === i}
        class:danger={item.danger}
        disabled={item.disabled}
        onmouseenter={() => (active = i)}
        onmouseleave={() => (active = -1)}
        onclick={() => run(item)}
      >{item.label}</button>
    {/each}
  </div>
{/if}

<style>
  .menu {
    --menu-fill: color-mix(in srgb, var(--accent) 8%, var(--surface-float));
    position: fixed;
    z-index: 50;
    display: flex;
    flex-direction: column;
    min-width: 170px;
    padding: 4px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background:
      linear-gradient(var(--menu-fill), var(--menu-fill)) padding-box,
      var(--edge-lit) border-box;
    backdrop-filter: blur(var(--glass-blur));
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.4);
    outline: none;
  }
  .menu button {
    text-align: left;
    padding: 6px 10px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .menu button.active:not(:disabled) {
    background: var(--surface-2);
  }
  .menu button.danger {
    color: var(--danger);
  }
  .menu button:disabled {
    color: var(--text-muted);
    cursor: default;
  }
</style>
