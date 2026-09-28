<script lang="ts">
  import { api, type Mix } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import { playMix } from "../lib/mixPlay";
  import type { Toasts } from "../lib/toasts.svelte";

  let { toasts }: { toasts: Toasts } = $props();

  let mixes = $state<Mix[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      mixes = await api.getFavoriteMixes();
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

<h2>Mixes &amp; Radio</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if mixes.length === 0}
  <EmptyState message="No mixes yet." />
{:else}
  <div class="grid">
    {#each mixes as mix (mix.id)}
      <button class="card" onclick={() => void playMix(mix.id, toasts)} title={`Play ${mix.title}`}>
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
