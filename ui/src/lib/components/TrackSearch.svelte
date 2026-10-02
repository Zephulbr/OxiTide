<script lang="ts">
  import type { TrackView } from "../trackView.svelte";
  import Icon from "./Icon.svelte";
  import Loading from "./Loading.svelte";

  let { view }: { view: TrackView } = $props();

  let open = $state(false);
  let root = $state<HTMLDivElement | null>(null);
  let input = $state<HTMLInputElement | null>(null);
  let toggleButton = $state<HTMLButtonElement | null>(null);

  $effect(() => {
    if (open) input?.focus();
  });

  // Clicking anywhere outside closes the search. `refocus` stays false there so
  // clicking a track does not yank focus back to the toggle.
  $effect(() => {
    if (!open) return;
    function onWindowMouseDown(event: MouseEvent) {
      if (root && event.target instanceof Node && root.contains(event.target)) return;
      close(false);
    }
    window.addEventListener("mousedown", onWindowMouseDown);
    return () => window.removeEventListener("mousedown", onWindowMouseDown);
  });

  /// Closing clears the filter so the table never stays filtered by a hidden
  /// control, and returns focus to the toggle; Escape does the same.
  function close(refocus = true) {
    view.setFilter("");
    open = false;
    if (refocus) toggleButton?.focus();
  }

  function toggle() {
    if (open) close();
    else open = true;
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    close();
  }
</script>

<div class="search-row" bind:this={root}>
  <div class="track-search" class:open>
    <button
      class="toggle"
      bind:this={toggleButton}
      onclick={toggle}
      aria-label={open ? "Close filter" : "Filter tracks"}
      title="Filter tracks"
    >
      <Icon name="search" size={18} />
    </button>
    <input
      bind:this={input}
      class="field"
      placeholder="Filter tracks"
      data-perf-filter
      tabindex={open ? 0 : -1}
      aria-hidden={!open}
      value={view.filter}
      oninput={(event) => view.setFilter(event.currentTarget.value)}
      onkeydown={onkeydown}
    />
    {#if view.filter.trim() !== ""}
      <button class="clear" onclick={() => view.setFilter("")} aria-label="Clear filter">
        <Icon name="x" size={14} />
      </button>
    {/if}
  </div>
  {#if view.preparing}
    <Loading label="Loading all tracks…" />
  {/if}
  {#if view.error}
    <span class="muted">{view.error}</span>
    <button onclick={() => view.retry()}>Retry</button>
  {/if}
</div>

<style>
  .search-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  /* One element that morphs from a 48px icon button into a search pill: the
     container's width transitions, the icon stays put, the field fades in. */
  .track-search {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
    width: 48px;
    height: 48px;
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    transition:
      width var(--dur-state) var(--ease-in-out),
      background var(--dur-state) var(--ease-in),
      border-color var(--dur-state) var(--ease-in),
      box-shadow var(--dur-state) var(--ease-in);
  }
  .track-search.open {
    width: min(240px, 26vw);
    border-color: color-mix(in srgb, var(--accent) 28%, var(--line));
    background: color-mix(in srgb, var(--accent) 8%, var(--surface-2));
    box-shadow: 0 0 10px color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .toggle {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 46px;
    height: 46px;
    border: 0;
    background: transparent;
    color: var(--text-muted);
    transition: color var(--dur-micro) var(--ease-in);
  }
  .toggle:hover,
  .track-search.open .toggle {
    color: var(--accent);
  }
  .field {
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    opacity: 0;
    transition: opacity var(--dur-state) var(--ease-in);
  }
  .field::placeholder {
    color: var(--text-muted);
  }
  .track-search.open .field {
    opacity: 1;
  }
  .clear {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    height: 100%;
    border: 0;
    background: transparent;
    color: var(--text-muted);
  }
  .clear:hover {
    color: var(--text);
  }
</style>
