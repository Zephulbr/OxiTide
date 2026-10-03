<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    value,
    options,
    label,
    onchange,
  }: {
    value: string;
    options: { value: string; label: string }[];
    label: string;
    onchange: (value: string) => void;
  } = $props();

  let open = $state(false);
  let active = $state(-1);
  let panel = $state<HTMLDivElement | null>(null);
  let button = $state<HTMLButtonElement | null>(null);

  const current = $derived(options.find((option) => option.value === value)?.label ?? "");

  function close(refocus = true) {
    open = false;
    active = -1;
    if (refocus) button?.focus();
  }

  function toggle() {
    if (open) {
      close();
      return;
    }
    active = options.findIndex((option) => option.value === value);
    open = true;
  }

  function choose(option: { value: string; label: string }) {
    onchange(option.value);
    close();
  }

  function move(delta: -1 | 1) {
    if (options.length === 0) return;
    active = (active + delta + options.length) % options.length;
  }

  function onKeydown(event: KeyboardEvent) {
    if (!open) {
      if (event.key === "ArrowDown" || event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        toggle();
      }
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      move(event.key === "ArrowDown" ? 1 : -1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      const option = options[active];
      if (option) choose(option);
    } else if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
  }

  // Focus the panel so arrows/Enter/Escape stay inside the dropdown.
  $effect(() => {
    if (open && panel) panel.focus();
  });

  function onWindowMouseDown(event: MouseEvent) {
    if (!open) return;
    if (panel && event.target instanceof Node && panel.contains(event.target)) return;
    if (button && event.target instanceof Node && button.contains(event.target)) return;
    close(false);
  }
</script>

<svelte:window onmousedown={onWindowMouseDown} />

<div class="select-wrap">
  <button
    bind:this={button}
    type="button"
    class="search-input select"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={label}
    onclick={toggle}
    onkeydown={onKeydown}
  >
    <span class="value">{current}</span>
    <span class="chevron" aria-hidden="true"><Icon name="chevron-down" size={14} /></span>
  </button>
  {#if open}
    <div
      bind:this={panel}
      class="panel"
      role="listbox"
      tabindex="-1"
      aria-label={label}
      onkeydown={onKeydown}
    >
      {#each options as option, i (option.value)}
        <button
          type="button"
          role="option"
          aria-selected={option.value === value}
          class:active={active === i}
          class:selected={option.value === value}
          onmouseenter={() => (active = i)}
          onmouseleave={() => (active = -1)}
          onclick={() => choose(option)}
        >
          <span class="label">{option.label}</span>
          {#if option.value === value}<Icon name="check" size={14} />{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .select-wrap {
    position: relative;
    display: flex;
    flex: 0 0 auto;
  }
  .select {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
  }
  .chevron {
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }
  /* The floating-overlay recipe shared with ContextMenu and the Explore
     suggestions, so the dropdown reads as one overlay family. */
  .panel {
    --menu-fill: color-mix(in srgb, var(--accent) 8%, var(--surface-float));
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 50;
    display: flex;
    flex-direction: column;
    min-width: 100%;
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
  .panel button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 10px;
    border-radius: 4px;
    text-align: left;
    white-space: nowrap;
  }
  .panel button :global(svg) {
    flex: 0 0 auto;
    color: var(--accent);
  }
  .panel button.active:not(.selected) {
    background: var(--surface-2);
  }
  .panel button.selected {
    background: var(--surface-2);
    font-weight: 600;
  }
</style>
