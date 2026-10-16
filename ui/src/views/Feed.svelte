<script lang="ts">
  import { untrack } from "svelte";
  import type { FeedActivity } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import FeedTimeline from "../lib/components/FeedTimeline.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import { load as loadCached, peek } from "../lib/viewCache";
  import { feedSource } from "../lib/viewData";

  let { nav }: { nav: Nav } = $props();

  // Cached visits render complete on the first frame; content that had to
  // load rises in (`.arrive`) with its count instead of snapping in.
  const source = feedSource();
  const cached = peek(source);
  const arrive = cached === undefined;
  let activities = $state<FeedActivity[]>(cached ?? []);
  let loading = $state(cached === undefined);
  let error = $state("");

  async function load() {
    if (peek(source) === undefined) loading = true;
    error = "";
    try {
      await loadCached(source, (feed) => (activities = feed));
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    untrack(load);
  });
</script>

<div class="feed-page">
  <header class="page-head">
    <h2>Feed</h2>
    {#if !loading && !error && activities.length > 0}
      <span class="muted count tnum" class:arrive>{activities.length} {activities.length === 1 ? "release" : "releases"}</span>
    {/if}
  </header>
  {#if loading}
    <Loading />
  {:else if error}
    <ErrorState message={error} onRetry={load} />
  {:else if activities.length === 0}
    <EmptyState message="No updates yet." />
  {:else}
    <div class:arrive><FeedTimeline {activities} {nav} /></div>
  {/if}
</div>

<style>
  .feed-page {
    padding-bottom: 32px;
  }
</style>
