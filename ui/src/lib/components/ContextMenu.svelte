<script lang="ts">
  import type { Menu, MenuItem } from "../menu.svelte";
  import Icon from "./Icon.svelte";

  let { menu }: { menu: Menu } = $props();

  let panel = $state<HTMLDivElement | null>(null);
  let left = $state(0);
  let top = $state(0);
  let active = $state(-1);
  // Menus where only some rows carry an icon (the sort/filter check) keep the
  // labels aligned with a spacer.
  let hasLeadIcons = $derived(menu.open?.items.some((item) => item.icon) ?? false);

  function inert(item: MenuItem): boolean {
    return Boolean(item.disabled || item.note);
  }

  function enabledIndexes(items: MenuItem[]): number[] {
    return items.map((item, index) => (inert(item) ? -1 : index)).filter((i) => i >= 0);
  }

  function clampToViewport() {
    const node = panel;
    const state = menu.open;
    if (!node || !state) return;
    const rect = node.getBoundingClientRect();
    const x = state.align === "end" ? state.x - rect.width : state.x;
    left = Math.max(4, Math.min(x, window.innerWidth - rect.width - 4));
    const y = state.placement === "above" ? state.y - rect.height : state.y;
    top = Math.max(4, Math.min(y, window.innerHeight - rect.height - 4));
  }

  function moveTo(position: "first" | "last") {
    const state = menu.open;
    if (!state) return;
    const enabled = enabledIndexes(state.items);
    if (enabled.length === 0) return;
    active = position === "first" ? enabled[0] : enabled[enabled.length - 1];
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
    if (inert(item)) return;
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
    const close = () => menu.close(false);
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
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      event.stopPropagation();
      moveTo(event.key === "Home" ? "first" : "last");
    } else if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      const item = active >= 0 ? state.items[active] : undefined;
      if (item) run(item);
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      menu.close();
    } else if (event.key === "Tab") {
      // No preventDefault: closing restores focus to the opener first, then
      // the default Tab action continues from there.
      menu.close();
    }
  }

  function onWindowMouseDown(event: MouseEvent) {
    if (!menu.open) return;
    if (panel && event.target instanceof Node && panel.contains(event.target)) return;
    // The pointer is choosing a new focus target; do not pull focus back.
    menu.close(false);
  }
</script>

<svelte:window onmousedown={onWindowMouseDown} onresize={() => menu.close(false)} />

{#if menu.open}
  <div
    bind:this={panel}
    class="menu"
    class:above={menu.open.placement === "above"}
    class:end={menu.open.align === "end"}
    role="menu"
    tabindex="-1"
    aria-activedescendant={active >= 0 && menu.open.items[active]
      ? `menu-item-${menu.open.items[active].id}`
      : undefined}
    style="left:{left}px;top:{top}px"
    onkeydown={onKeydown}
  >
    {#each menu.open.items as item, i (item.id)}
      {#if item.section && item.section !== menu.open.items[i - 1]?.section}
        <p class="section" class:first={i === 0}>{item.section}</p>
      {/if}
      {#if item.note}
        <!-- A status line: not a (disabled) button, so it is never dimmed. -->
        <div
          id={`menu-item-${item.id}`}
          class="note"
          class:divided={item.divider && i > 0 && !item.section}
          role="menuitem"
          aria-disabled="true"
          tabindex="-1"
        >
          {#if item.content}
            {@render item.content()}
          {:else}
            <span class="label">{item.label}{#if item.detail}<span class="detail">{item.detail}</span>{/if}</span>
          {/if}
        </div>
      {:else}
      <button
        id={`menu-item-${item.id}`}
        type="button"
        role="menuitem"
        tabindex="-1"
        class:active={active === i}
        class:selected={item.selected}
        class:danger={item.danger}
        class:divided={item.divider && i > 0 && !item.section}
        disabled={item.disabled}
        onmouseenter={() => (active = i)}
        onmouseleave={() => (active = -1)}
        onclick={() => run(item)}
      >
        {#if item.icon}
          <Icon name={item.icon} size={16} />
        {:else if hasLeadIcons}
          <span class="lead-spacer" aria-hidden="true"></span>
        {/if}
        {#if item.detail}
          <span class="label">{item.label}<span class="detail">{item.detail}</span></span>
        {:else}
          <span class="label">{item.label}</span>
        {/if}
        {#if item.trailing}<Icon name={item.trailing} size={14} />{/if}
      </button>
      {/if}
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
    /* Sized by its content, not by the room right of where it opens: a menu
       opened near the right edge is moved left by the clamp, not squeezed. */
    width: max-content;
    min-width: 196px;
    max-width: calc(100vw - 8px);
    padding: 6px;
    border: 1px solid transparent;
    border-radius: var(--radius-card);
    background:
      linear-gradient(var(--menu-fill), var(--menu-fill)) padding-box,
      var(--edge-lit) border-box;
    backdrop-filter: blur(var(--glass-blur));
    box-shadow:
      0 16px 40px rgb(0 0 0 / 0.45),
      0 2px 6px rgb(0 0 0 / 0.3);
    outline: none;
    transform-origin: top left;
    animation: menu-in var(--dur-state) var(--ease-enter);
  }
  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.97);
    }
  }
  /* Grown upward from a bottom-edge opener: it unfolds from its foot. */
  .menu.above {
    transform-origin: bottom left;
    animation-name: menu-in-above;
  }
  .menu.end {
    transform-origin: top right;
  }
  .menu.above.end {
    transform-origin: bottom right;
  }
  @keyframes menu-in-above {
    from {
      opacity: 0;
      transform: translateY(4px) scale(0.97);
    }
  }
  .section {
    margin: 0;
    padding: 8px 10px 3px;
    color: var(--text-muted);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .section:not(.first) {
    margin-top: 5px;
    border-top: 1px solid var(--line);
    padding-top: 9px;
  }
  .menu button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 11px;
    text-align: left;
    padding: 8px 11px;
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    color: inherit;
    font-size: 14px;
    cursor: pointer;
    transition:
      background-color var(--dur-micro) var(--ease-in-out),
      color var(--dur-micro) var(--ease-in-out);
  }
  /* A hairline between groups: the gap is the button margin so the divider
     sits centered in it and the hover wash never touches it. */
  .menu button.divided {
    position: relative;
    margin-top: 7px;
  }
  .menu button.divided::before {
    content: "";
    position: absolute;
    left: 6px;
    right: 6px;
    top: -4px;
    height: 1px;
    background: var(--line);
  }
  .menu button + button {
    margin-top: 1px;
  }
  .lead-spacer {
    flex: 0 0 16px;
    width: 16px;
  }
  .menu button .label {
    flex: 1;
    min-width: 0;
  }
  .menu button .detail {
    display: block;
    margin-top: 1px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 400;
  }
  .menu button :global(svg) {
    flex: 0 0 auto;
    color: var(--text-muted);
    transition: color var(--dur-micro) var(--ease-in-out);
  }
  .menu button.active:not(:disabled) {
    background: var(--surface-2);
  }
  .menu button.active:not(:disabled) :global(svg) {
    color: var(--accent);
  }
  /* Selected (the current sort/filter) is told apart from the hover wash by
     the accent, not a second background. */
  .menu button.selected {
    color: var(--accent);
    font-weight: 600;
  }
  .menu button.selected :global(svg) {
    color: var(--accent);
  }
  .menu button.danger,
  .menu button.danger :global(svg) {
    color: var(--danger);
  }
  .menu button.danger.active:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }
  .menu button:disabled :global(svg) {
    opacity: 0.5;
  }
  .menu button:disabled {
    color: var(--text-muted);
    cursor: default;
  }
  /* A status line reads as information, not as a greyed-out action: the
     row's padding and type, no hover wash. */
  .note {
    display: flex;
    padding: 8px 11px;
    font-size: 13px;
    cursor: default;
  }
  .note .label {
    flex: 1;
    min-width: 0;
  }
  .note .detail {
    display: block;
    margin-top: 1px;
    color: var(--text-muted);
    font-size: 12px;
  }
  .note.divided {
    position: relative;
    margin-top: 7px;
  }
  .note.divided::before {
    content: "";
    position: absolute;
    left: 6px;
    right: 6px;
    top: -4px;
    height: 1px;
    background: var(--line);
  }
</style>
