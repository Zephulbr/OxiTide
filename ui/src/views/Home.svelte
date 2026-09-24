<script lang="ts">
  import { api, type HomeSection } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import { routeForHomeItem } from "../lib/nav.svelte";
  import type { Nav } from "../lib/nav.svelte";
  let { nav }: { nav: Nav } = $props();
  let sections = $state<HomeSection[]>([]);
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
    <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(140px,1fr));gap:12px">
      {#each section.items as item, i (item.id || `${item.title}-${i}`)}
        {@const route = routeForHomeItem(item)}
        {#if route}
          <button
            style="text-align:left;background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:10px;color:inherit"
            onclick={() => nav.goto(route)}
          >
            <div>{item.title}</div>
            {#if item.subtitle}<div class="muted" style="font-size:12px">{item.subtitle}</div>{/if}
          </button>
        {:else}
          <div style="text-align:left;background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:10px;color:inherit">
            <div>{item.title}</div>
            {#if item.subtitle}<div class="muted" style="font-size:12px">{item.subtitle}</div>{/if}
          </div>
        {/if}
      {/each}
    </div>
  {/each}
{/if}
