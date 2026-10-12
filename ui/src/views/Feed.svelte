<script lang="ts">
  import { api, type FeedActivity } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import FeedTimeline from "../lib/components/FeedTimeline.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import type { Nav } from "../lib/nav.svelte";

  let { nav }: { nav: Nav } = $props();

  let activities = $state<FeedActivity[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      activities = await api.getFeed();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });
</script>

<div class="feed-page">
  <header class="page-head">
    <h2>Feed</h2>
    {#if !loading && !error && activities.length > 0}
      <span class="muted count tnum">{activities.length} {activities.length === 1 ? "release" : "releases"}</span>
    {/if}
  </header>
  {#if loading}
    <Loading />
  {:else if error}
    <ErrorState message={error} onRetry={load} />
  {:else if activities.length === 0}
    <EmptyState message="No updates yet." />
  {:else}
    <FeedTimeline {activities} {nav} />
  {/if}
</div>

<style>
  .feed-page {
    padding-bottom: 32px;
  }
</style>
