<script lang="ts">
  import { api, type PageSection } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import { routeForHomeItem } from "../lib/nav.svelte";
  import type { Nav } from "../lib/nav.svelte";
  let { nav }: { nav: Nav } = $props();
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
  {#each sections as section, si (`${section.title}-${si}`)}
    <h3>{section.title}</h3>
    <div class="card-grid">
      {#each section.items as item, i (item.id || `${item.title}-${i}`)}
        {@const route = routeForHomeItem(item)}
        {#if route}
          <button class="card" onclick={() => nav.goto(route)}>
            <span class="card-art"><Cover uuid={item.artwork} size={320} radius={12} alt="" /></span>
            <span class="card-title">{item.title}</span>
            {#if item.subtitle}<span class="muted card-sub">{item.subtitle}</span>{/if}
          </button>
        {:else}
          <div class="card inert">
            <span class="card-art"><Cover uuid={item.artwork} size={320} radius={12} alt="" /></span>
            <span class="card-title">{item.title}</span>
            {#if item.subtitle}<span class="muted card-sub">{item.subtitle}</span>{/if}
          </div>
        {/if}
      {/each}
    </div>
  {/each}
{/if}

<style>
  .card-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
    margin-bottom: 20px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
    padding: 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    color: inherit;
  }
  .card:hover {
    background: var(--surface-2);
  }
  .card-art {
    display: block;
    overflow: hidden;
    border-radius: var(--radius-card);
  }
  .card-art :global(img) {
    transition: transform var(--dur-state) var(--ease-in);
  }
  .card:hover .card-art :global(img) {
    transform: scale(1.03);
  }
  .card-title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card-sub {
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card.inert {
    opacity: 0.9;
  }
</style>
