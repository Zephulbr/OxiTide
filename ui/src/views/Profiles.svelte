<script lang="ts">
  import { api, type Artist } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import type { Nav } from "../lib/nav.svelte";

  let { nav }: { nav: Nav } = $props();

  let artists = $state<Artist[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      artists = await api.getFavoriteArtists();
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

<h2>Profiles</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if artists.length === 0}
  <EmptyState message="No artists yet." />
{:else}
  <div class="grid">
    {#each artists as artist (artist.id)}
      <button class="card" onclick={() => nav.goto({ name: "artist", id: artist.id })}>
        <span class="art"><Cover uuid={artist.picture} size={320} radius={160} alt="" /></span>
        <span class="title">{artist.name}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 20px;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    text-align: center;
    padding: 12px;
    border-radius: var(--radius-card);
    color: inherit;
  }
  .card:hover {
    background: var(--surface-2);
  }
  .art {
    display: block;
    width: 100%;
    overflow: hidden;
    border-radius: 50%;
  }
  .title {
    font-weight: 500;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
