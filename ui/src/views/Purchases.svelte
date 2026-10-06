<script lang="ts">
  import { api, type PageItem } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import { routeForPageItem, type Nav } from "../lib/nav.svelte";

  let { nav }: { nav: Nav } = $props();

  let items = $state<PageItem[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      items = await api.getPurchases();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void load();
  });
</script>

<h2>Purchases</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if items.length === 0}
  <EmptyState message="Nothing purchased on this account." />
{:else}
  <div class="purchases">
    {#each items as item (item.id || item.title)}
      {@const route = routeForPageItem(item)}
      <MediaCard
        item={item}
        variant="row"
        onclick={route ? () => nav.goto(route) : undefined}
      />
    {/each}
  </div>
{/if}

<style>
  .purchases {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
    gap: 8px 20px;
  }
</style>
