<script lang="ts">
  import { api, type PageItem } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import type { Route } from "../lib/nav.svelte";
  import type { Nav } from "../lib/nav.svelte";

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

  function routeFor(item: PageItem): Route | null {
    if (item.item_type.toUpperCase().includes("ALBUM") && /^\d+$/.test(item.id)) {
      return { name: "album", id: Number(item.id) };
    }
    return null;
  }
</script>

<h2>Purchases</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if items.length === 0}
  <EmptyState message="Nothing purchased on this account." />
{:else}
  <ul class="purchases">
    {#each items as item (item.id || item.title)}
      {@const route = routeFor(item)}
      <li>
        <span class="thumb"><Cover uuid={item.artwork} size={80} radius={6} alt="" /></span>
        {#if route}
          <button onclick={() => nav.goto(route)}>{item.title}</button>
        {:else}
          <span>{item.title}</span>
        {/if}
        <span class="muted sub">{item.item_type}</span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .purchases {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .purchases li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
  }
  .thumb {
    flex: 0 0 auto;
    width: 40px;
  }
  .sub {
    margin-left: auto;
    font-size: 11.5px;
  }
</style>
