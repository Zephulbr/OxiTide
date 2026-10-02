<script lang="ts">
  import { api, type PageSection } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let { nav, toasts }: { nav: Nav; toasts: Toasts } = $props();

  let sections = $state<PageSection[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      sections = await api.getHome();
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

<h2>Home</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if sections.length === 0}
  <EmptyState message="Nothing to show yet." />
{:else}
  <PageSections {sections} {nav} {toasts} source="home" />
{/if}
