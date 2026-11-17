<script lang="ts" generics="T extends string">
  import { filterOptions } from "../../optionFilter";
  import Icon from "../Icon.svelte";

  /// A dropdown in the shared field family (same trigger as `SortSelect`,
  /// same floating-panel recipe) for settings with more than a few choices or
  /// choices that need a line of explanation. Full listbox keyboard pattern;
  /// the panel flips upward near the bottom of the window. With `search`, a
  /// filter field heads the panel (combobox pattern: focus stays in the
  /// field, the arrows move through the matches).
  type Option = { value: T; label: string; hint?: string; disabled?: boolean; keywords?: readonly string[] };
  let {
    value,
    options,
    label,
    onchange,
    placeholder = "",
    width = 220,
    disabled = false,
    search,
  }: {
    value: T;
    options: Option[];
    label: string;
    onchange: (value: T) => void;
    placeholder?: string;
    width?: number;
    disabled?: boolean;
    /// The filter field's label (also its placeholder) and the no-match text.
    search?: { label: string; empty: string };
  } = $props();

  const uid = $props.id();
  let open = $state(false);
  let active = $state(-1);
  let flipUp = $state(false);
  let query = $state("");
  let panel = $state<HTMLDivElement | null>(null);
  let list = $state<HTMLDivElement | null>(null);
  let field = $state<HTMLInputElement | null>(null);
  let button = $state<HTMLButtonElement | null>(null);

  const current = $derived(options.find((option) => option.value === value)?.label ?? placeholder);
  const visible = $derived(search ? filterOptions(options, query) : options);
  const activeId = $derived(active >= 0 && visible[active] ? `${uid}-${active}` : undefined);

  function close(refocus = true) {
    open = false;
    active = -1;
    query = "";
    if (refocus) button?.focus();
  }

  function toggle() {
    if (open) {
      close();
      return;
    }
    query = "";
    active = options.findIndex((option) => option.value === value);
    if (button) {
      const rect = button.getBoundingClientRect();
      const wanted = Math.min(options.length * 40 + 12, 320) + (search ? 44 : 0);
      const below = window.innerHeight - rect.bottom;
      flipUp = below < wanted && rect.top > below;
    }
    open = true;
  }

  function onQuery() {
    // The first selectable match is active, so Enter picks it.
    active = visible.findIndex((option) => !option.disabled);
    if (list) list.scrollTop = 0;
  }

  function choose(option: Option) {
    if (option.disabled) return;
    onchange(option.value);
    close();
  }

  function move(delta: -1 | 1) {
    if (visible.length === 0) return;
    let next = active;
    for (let i = 0; i < visible.length; i += 1) {
      next = (next + delta + visible.length) % visible.length;
      if (!visible[next].disabled) break;
    }
    active = next;
    list?.querySelectorAll<HTMLElement>("[role=option]")[next]?.scrollIntoView({ block: "nearest" });
  }

  function onKeydown(event: KeyboardEvent) {
    // In the filter field, Space, Home and End edit the text.
    if (event.target === field && (event.key === " " || event.key === "Home" || event.key === "End")) {
      event.stopPropagation();
      return;
    }
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
      if (visible.length > 0) active = event.key === "Home" ? 0 : visible.length - 1;
    } else if (event.key === "Enter") {
      event.preventDefault();
      const option = visible[active];
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
    if (!open || !list) return;
    (field ?? list).focus();
    list.querySelector<HTMLElement>(".selected")?.scrollIntoView({ block: "nearest" });
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
    <div bind:this={panel} class="panel" class:up={flipUp}>
      {#if search}
        <label class="filter">
          <Icon name="search" size={14} />
          <input
            bind:this={field}
            bind:value={query}
            type="text"
            role="combobox"
            autocomplete="off"
            spellcheck="false"
            aria-expanded="true"
            aria-autocomplete="list"
            aria-controls={`${uid}-list`}
            aria-activedescendant={activeId}
            aria-label={search.label}
            placeholder={search.label}
            oninput={onQuery}
            onkeydown={onKeydown}
          />
        </label>
      {/if}
      <div
        bind:this={list}
        id={`${uid}-list`}
        class="list"
        role="listbox"
        tabindex="-1"
        aria-label={label}
        aria-activedescendant={search ? undefined : activeId}
        onkeydown={onKeydown}
      >
        {#each visible as option, i (option.value)}
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
      {#if search && visible.length === 0}
        <p class="empty" role="status">{search.empty}</p>
      {/if}
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
    max-height: 364px;
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
  .list {
    display: flex;
    flex-direction: column;
    max-height: 312px;
    overflow-y: auto;
    outline: none;
  }
  /* The filter field: a quiet row above the options, not a second input box. */
  .filter {
    display: flex;
    flex: none;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
    color: var(--text-muted);
    cursor: text;
  }
  .filter input {
    flex: 1 1 auto;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    outline: none;
  }
  .filter input::placeholder {
    color: var(--text-muted);
  }
  .filter:focus-within {
    border-bottom-color: var(--accent);
  }
  .empty {
    margin: 0;
    padding: 8px 10px;
    color: var(--text-muted);
    font-size: 12.5px;
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
  /* Windows contrast themes (A-3): the chosen option takes the system
     selection colours, the keyboard-active one a frame. */
  @media (forced-colors: active) {
    .panel button.selected {
      forced-color-adjust: none;
      background: Highlight;
      color: HighlightText;
    }
    .panel button.active:not(.selected) {
      outline: 1px solid Highlight;
      outline-offset: -1px;
    }
  }
</style>
