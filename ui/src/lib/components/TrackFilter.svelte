<script lang="ts">
  import type { TrackView } from "../trackView.svelte";
  import Icon from "./Icon.svelte";
  import Loading from "./Loading.svelte";
  import { t } from "../i18n.svelte";

  let { view }: { view: TrackView } = $props();
</script>

<div class="track-filter">
  <input
    class="filter search-input"
    placeholder={t("filter.tracks")}
    aria-label={t("filter.tracks")}
    data-perf-filter
    value={view.filter}
    oninput={(event) => view.setFilter(event.currentTarget.value)}
  />
  {#if view.filter.trim() !== ""}
    <button onclick={() => view.setFilter("")} aria-label={t("filter.clear")}><Icon name="x" size={14} /></button>
  {/if}
  {#if view.preparing}
    <Loading label={t("filter.loadingAll")} />
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
