<script lang="ts">
  import { SETTINGS_TABS } from "../../settingsTabs";
  import { searchSettings, type SearchContext, type SettingEntry } from "../../settingsIndex";
  import { TAB_META } from "../../settingsView";
  import Icon from "../Icon.svelte";

  /// Page-header search over every setting. A combobox: type, arrow to a
  /// result, Enter (or click) to jump to it; Escape clears.
  let {
    scope,
    onselect,
  }: {
    /// What is on the page right now (Rack options, the exclusive-mode fallback).
    scope: SearchContext;
    onselect: (entry: SettingEntry) => void;
  } = $props();

  const uid = $props.id();
  let query = $state("");
  let open = $state(false);
  let active = $state(0);
  let input = $state<HTMLInputElement | null>(null);
  let root = $state<HTMLDivElement | null>(null);

  const results = $derived(searchSettings(query, scope));
  const showing = $derived(open && query.trim().length > 0);

  // A new query restarts the highlight at the best match.
  $effect(() => {
    query;
    active = 0;
  });

  function choose(entry: SettingEntry) {
    onselect(entry);
    query = "";
    open = false;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      // Arrows must not reach the global volume/seek shortcuts.
      event.stopPropagation();
      event.preventDefault();
      if (!showing) {
        open = true;
        return;
      }
      if (results.length === 0) return;
      active = (active + (event.key === "ArrowDown" ? 1 : -1) + results.length) % results.length;
    } else if (event.key === "Enter") {
      const entry = results[active];
      if (showing && entry) {
        event.preventDefault();
        choose(entry);
      }
    } else if (event.key === "Escape") {
      // Clear first; a bare Escape still falls through to the app's layers.
      if (query || open) {
        event.stopPropagation();
        query = "";
        open = false;
      }
    }
  }

  function onWindowMouseDown(event: MouseEvent) {
    if (open && root && event.target instanceof Node && !root.contains(event.target)) open = false;
  }
</script>

<svelte:window onmousedown={onWindowMouseDown} />

<div class="settings-search" bind:this={root}>
  <span class="glass" aria-hidden="true"><Icon name="search" size={15} /></span>
  <input
    bind:this={input}
    bind:value={query}
    class="search-input"
    type="text"
    role="combobox"
    placeholder="Search settings"
    aria-label="Search settings"
    aria-autocomplete="list"
    aria-expanded={showing}
    aria-controls={`${uid}-list`}
    aria-activedescendant={showing && results[active] ? `${uid}-${active}` : undefined}
    autocomplete="off"
    spellcheck="false"
    onfocus={() => (open = true)}
    oninput={() => (open = true)}
    onkeydown={onKeydown}
  />
  {#if query}
    <button
      type="button"
      class="clear"
      aria-label="Clear search"
      onclick={() => {
        query = "";
        input?.focus();
      }}
    ><Icon name="x" size={14} /></button>
  {/if}
  {#if showing}
    <div class="panel" id={`${uid}-list`} role={results.length > 0 ? "listbox" : undefined} aria-label="Matching settings">
      {#each results as entry, i (entry.id)}
        <button
          id={`${uid}-${i}`}
          type="button"
          role="option"
          tabindex="-1"
          aria-selected={i === active}
          class:active={i === active}
          onmouseenter={() => (active = i)}
          onmousedown={(event) => event.preventDefault()}
          onclick={() => choose(entry)}
        >
          <span class="tab-icon" aria-hidden="true"><Icon name={TAB_META[entry.tab].icon} size={15} /></span>
          <span class="text">
            <span class="label">{entry.label}</span>
            <span class="where">{TAB_META[entry.tab].label} · {entry.section}</span>
          </span>
        </button>
      {:else}
        <p class="none" role="status">
          No settings match “{query.trim()}”
        </p>
      {/each}
    </div>
  {/if}
</div>

<style>
  .settings-search {
    position: relative;
    width: min(420px, 100%);
    justify-self: center;
  }
  .glass {
    position: absolute;
    top: 50%;
    left: 12px;
    display: grid;
    color: var(--text-muted);
    transform: translateY(-50%);
    pointer-events: none;
  }
  input {
    width: 100%;
    height: 38px;
    padding: 0 34px 0 36px;
    font-size: 13.5px;
  }
  .clear {
    position: absolute;
    top: 50%;
    right: 6px;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    color: var(--text-muted);
    transform: translateY(-50%);
  }
  .clear:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  /* The floating-overlay recipe shared with ContextMenu, SortSelect and Select. */
  .panel {
    --menu-fill: color-mix(in srgb, var(--accent) 8%, var(--surface-float));
    position: absolute;
    top: calc(100% + 6px);
    left: 50%;
    z-index: 50;
    display: flex;
    flex-direction: column;
    width: max(100%, 380px);
    max-width: 100vw;
    max-height: 380px;
    transform: translateX(-50%);
    overflow-y: auto;
    padding: 4px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background:
      linear-gradient(var(--menu-fill), var(--menu-fill)) padding-box,
      var(--edge-lit) border-box;
    backdrop-filter: blur(var(--glass-blur));
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.4);
  }
  .panel button {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 4px;
    text-align: left;
  }
  .panel button.active {
    background: var(--surface-2);
  }
  .tab-icon {
    display: grid;
    flex: none;
    color: var(--accent);
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .label {
    overflow: hidden;
    font-weight: 550;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .where {
    color: var(--text-muted);
    font-size: 12px;
  }
  .none {
    margin: 0;
    padding: 12px;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
