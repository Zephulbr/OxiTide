<script lang="ts">
  import { api, type Mix, type PageSection } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import type { Nav } from "../lib/nav.svelte";

  let { nav }: { nav: Nav } = $props();

  let forYou = $state<PageSection[]>([]);
  let mixes = $state<Mix[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    const [forYouResult, mixesResult] = await Promise.allSettled([
      api.getForYou(),
      api.getFavoriteMixes(),
    ]);
    forYou = forYouResult.status === "fulfilled" ? forYouResult.value : [];
    if (mixesResult.status === "fulfilled") {
      mixes = mixesResult.value;
    } else {
      error = String(mixesResult.reason);
    }
    loading = false;
  }

  $effect(() => {
    void load();
  });
</script>

<h2>Mixes &amp; Radio</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if forYou.length === 0 && mixes.length === 0}
  <EmptyState message="No mixes yet." />
{:else}
  {#if forYou.length > 0}
    <PageSections sections={forYou} {nav} />
  {/if}
  {#if mixes.length > 0}
    <h3>Favorite mixes</h3>
    <div class="grid">
      {#each mixes as mix (mix.id)}
        <button class="card" onclick={() => nav.goto({ name: "mix", id: mix.id })} title={`Open ${mix.title}`}>
          <span class="art">
            {#if mix.image}
              <img src={mix.image} alt="" loading="lazy" />
            {:else}
              <span class="placeholder"><Icon name="disc-3" size={28} /></span>
            {/if}
          </span>
          <span class="title">{mix.title}</span>
          {#if mix.subTitle}<span class="muted sub">{mix.subTitle}</span>{/if}
        </button>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
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
  .art {
    display: grid;
    place-items: center;
    aspect-ratio: 1;
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--surface-2);
    color: var(--text-muted);
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
