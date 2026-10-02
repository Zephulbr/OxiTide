<script lang="ts">
  import type { TrackView } from "../trackView.svelte";
  import Icon from "./Icon.svelte";
  import Loading from "./Loading.svelte";

  let { view }: { view: TrackView } = $props();
</script>

<div class="track-filter">
  <input
    class="filter search-input"
    placeholder="Filter tracks"
    data-perf-filter
    value={view.filter}
    oninput={(event) => view.setFilter(event.currentTarget.value)}
  />
  {#if view.filter.trim() !== ""}
    <button onclick={() => view.setFilter("")} aria-label="Clear filter"><Icon name="x" size={14} /></button>
  {/if}
  {#if view.preparing}
    <Loading label="Loading all tracks…" />
  {/if}
  {#if view.error}
    <span class="muted">{view.error}</span>
    <button onclick={() => view.retry()}>Retry</button>
  {/if}
</div>

<style>
  .track-filter {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .filter {
    flex: 0 1 260px;
  }
</style>
