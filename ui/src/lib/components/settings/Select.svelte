<script lang="ts" generics="T extends string">
  import Icon from "../Icon.svelte";

  /// A dropdown in the shared field family (same trigger as `SortSelect`,
  /// same floating-panel recipe) for settings with more than a few choices or
  /// choices that need a line of explanation. Full listbox keyboard pattern;
  /// the panel flips upward near the bottom of the window.
  let {
    value,
    options,
    label,
    onchange,
    placeholder = "",
    width = 220,
    disabled = false,
  }: {
    value: T;
    options: { value: T; label: string; hint?: string; disabled?: boolean }[];
    label: string;
    onchange: (value: T) => void;
    placeholder?: string;
    width?: number;
    disabled?: boolean;
  } = $props();

  const uid = $props.id();
  let open = $state(false);
  let active = $state(-1);
  let flipUp = $state(false);
  let panel = $state<HTMLDivElement | null>(null);
  let button = $state<HTMLButtonElement | null>(null);

  const current = $derived(options.find((option) => option.value === value)?.label ?? placeholder);

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
    if (button) {
      const rect = button.getBoundingClientRect();
      const wanted = Math.min(options.length * 40 + 12, 320);
      const below = window.innerHeight - rect.bottom;
      flipUp = below < wanted && rect.top > below;
    }
    open = true;
  }

  function choose(option: (typeof options)[number]) {
    if (option.disabled) return;
    onchange(option.value);
    close();
  }

  function move(delta: -1 | 1) {
    if (options.length === 0) return;
    let next = active;
    for (let i = 0; i < options.length; i += 1) {
      next = (next + delta + options.length) % options.length;
      if (!options[next].disabled) break;
    }
    active = next;
  }

  function onKeydown(event: KeyboardEvent) {
    const handled = ["ArrowDown", "ArrowUp", "Home", "End", "Enter", " ", "Escape", "Tab"];
    if (!handled.includes(event.key)) return;
    // The arrows must not reach the global volume/seek shortcuts.
    if (event.key !== "Tab") event.stopPropagation();
    if (!open) {
      if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        toggle();
      }
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      move(event.key === "ArrowDown" ? 1 : -1);
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      if (options.length > 0) active = event.key === "Home" ? 0 : options.length - 1;
    } else if (event.key === "Enter") {
      event.preventDefault();
      const option = options[active];
      if (option) choose(option);
    } else if (event.key === "Escape") {
      event.preventDefault();
      close();
    } else if (event.key === "Tab") {
      close();
    }
  }

  // Focus the panel so arrows/Enter/Escape stay inside the dropdown, and bring
  // the selected option into view in long lists (output devices).
  $effect(() => {
    if (!open || !panel) return;
    panel.focus();
    panel.querySelector<HTMLElement>(".selected")?.scrollIntoView({ block: "nearest" });
  });

  function onWindowMouseDown(event: MouseEvent) {
    if (!open) return;
    if (panel && event.target instanceof Node && panel.contains(event.target)) return;
    if (button && event.target instanceof Node && button.contains(event.target)) return;
    close(false);
  }
</script>

<svelte:window onmousedown={onWindowMouseDown} />

<div class="select-wrap" style:width="{width}px" style:max-width="100%">
  <button
    bind:this={button}
    type="button"
    class="search-input select"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={label}
    {disabled}
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
      class:up={flipUp}
      role="listbox"
      tabindex="-1"
      aria-label={label}
      aria-activedescendant={active >= 0 && options[active] ? `${uid}-${active}` : undefined}
      onkeydown={onKeydown}
    >
      {#each options as option, i (option.value)}
        <button
          id={`${uid}-${i}`}
          type="button"
          role="option"
          tabindex="-1"
          disabled={option.disabled}
          aria-selected={option.value === value}
          class:active={active === i}
          class:selected={option.value === value}
          onmouseenter={() => (active = i)}
          onmouseleave={() => (active = -1)}
          onclick={() => choose(option)}
        >
          <span class="text">
            <span class="label">{option.label}</span>
            {#if option.hint}<span class="hint">{option.hint}</span>{/if}
          </span>
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
    flex: 0 1 auto;
    min-width: 0;
  }
  .select {
    display: flex;
    flex: 1 1 auto;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
    padding: 7px 10px 7px 12px;
    font-size: 13.5px;
    text-align: left;
    cursor: pointer;
  }
  .value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chevron {
    display: grid;
    flex: none;
    place-items: center;
    color: var(--text-muted);
  }
  .select[aria-expanded="true"] .chevron {
    transform: rotate(180deg);
  }
  /* The floating-overlay recipe shared with ContextMenu and SortSelect. */
  .panel {
    --menu-fill: color-mix(in srgb, var(--accent) 8%, var(--surface-float));
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 50;
    display: flex;
    flex-direction: column;
    width: max-content;
    min-width: max(100%, 220px);
    max-width: 380px;
    max-height: 320px;
    overflow-y: auto;
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
  .panel.up {
    top: auto;
    bottom: calc(100% + 6px);
  }
  .panel button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 7px 10px;
    border-radius: 4px;
    text-align: left;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    color: var(--text-muted);
    font-size: 12px;
  }
  .panel button :global(svg) {
    flex: 0 0 auto;
    color: var(--accent);
  }
  .panel button.active:not(.selected),
  .panel button.selected {
    background: var(--surface-2);
  }
  .panel button.selected {
    font-weight: 600;
  }
</style>
