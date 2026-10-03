<script lang="ts">
  import { api, type Artist } from "../lib/api";
  import { visibleArtists, type ArtistSort } from "../lib/artistSort";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import type { Nav } from "../lib/nav.svelte";

  let { nav }: { nav: Nav } = $props();

  let artists = $state<Artist[]>([]);
  let loading = $state(true);
  let error = $state("");
  let query = $state("");
  let sortMode = $state<ArtistSort>("name");

  const shown = $derived(visibleArtists(artists, query, sortMode));

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

<div class="page-head">
  <h2>Profiles</h2>
  <span class="muted count">{artists.length} {artists.length === 1 ? "artist" : "artists"}</span>
  <input
    class="search-input filter"
    placeholder="Search artists"
    value={query}
    oninput={(event) => (query = event.currentTarget.value)}
    aria-label="Search artists"
  />
  <select
    class="search-input sort"
    value={sortMode}
    onchange={(event) => (sortMode = event.currentTarget.value as ArtistSort)}
    aria-label="Sort artists"
  >
    <option value="name">Name A–Z</option>
    <option value="name-desc">Name Z–A</option>
  </select>
</div>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if artists.length === 0}
  <EmptyState message="No artists yet." />
{:else if shown.length === 0}
  <p class="muted empty">No artists match “{query.trim()}”.</p>
{:else}
  <div class="grid">
    {#each shown as artist (artist.id)}
      <button class="card" onclick={() => nav.goto({ name: "artist", id: artist.id })}>
        <span class="art"><Cover uuid={artist.picture} size={320} radius={160} alt="" /></span>
        <span class="title">{artist.name}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .page-head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-top: 16px;
  }
  .page-head h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
  }
  .count {
    font-size: 13px;
  }
  .filter {
    flex: 0 1 240px;
  }
  .sort {
    flex: 0 0 auto;
  }
  .empty {
    margin-top: 40px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(8, minmax(0, 1fr));
    gap: 40px 20px;
    margin-top: 40px;
    padding: 0 16px;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
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
    font-size: 15px;
    font-weight: 500;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
