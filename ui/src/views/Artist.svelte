<script lang="ts">
  import { api, type Artist as ArtistT } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import { playArtistRadio } from "../lib/radio";
  import type { Toasts } from "../lib/toasts.svelte";

  let {
    id,
    nav,
    toasts,
    favorites,
  }: { id: number; nav: Nav; toasts: Toasts; favorites: Favorites } = $props();

  let artist = $state<ArtistT | null>(null);
  let similar = $state<ArtistT[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      artist = await api.getArtist(id);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
    try {
      similar = await api.getSimilarArtists(id);
    } catch {
      similar = [];
    }
  }

  $effect(() => {
    load();
  });
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if artist}
  {@const artistId = artist.id}
  <header class="view-header">
    <span class="header-art round"><Cover uuid={artist.picture} size={320} radius={80} alt={artist.name} /></span>
    <div class="header-meta">
      <h2>{artist.name}</h2>
      <p class="muted">Artist</p>
      <div class="header-actions">
        <button
          onclick={() => void playArtistRadio(artistId, toasts)}
          aria-label="Start artist radio"
          title="Start artist radio"
        ><Icon name="radio" /></button>
        <button
          class:on={favorites.hasArtist(artistId)}
          onclick={() => favorites.toggleArtist(artistId)}
          aria-label={favorites.hasArtist(artistId) ? "Unfavorite artist" : "Favorite artist"}
          title={favorites.hasArtist(artistId) ? "Unfavorite artist" : "Favorite artist"}
        ><Icon name="heart" /></button>
      </div>
    </div>
  </header>
  {#if similar.length > 0}
    <h3>Similar artists</h3>
    <div class="similar-grid">
      {#each similar as s (s.id)}
        <button class="similar-card" onclick={() => nav.goto({ name: "artist", id: s.id })} title={s.name}>
          <span class="similar-art"><Cover uuid={s.picture} size={320} radius={80} alt={s.name} /></span>
          <span class="similar-name">{s.name}</span>
        </button>
      {/each}
    </div>
  {/if}
{:else}
  <EmptyState message="Artist not found." />
{/if}

<style>
  .on { color: var(--accent); }
  .on :global(svg) { fill: currentColor; }
  .view-header {
    display: flex;
    align-items: flex-end;
    gap: 20px;
    margin-bottom: 20px;
  }
  .header-art {
    flex: 0 0 auto;
    width: 160px;
  }
  .header-meta {
    min-width: 0;
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }
  .similar-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 16px;
  }
  .similar-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    color: inherit;
    text-align: center;
  }
  .similar-card:hover {
    background: var(--surface-2);
  }
  .similar-art {
    display: block;
    width: 100%;
  }
  .similar-name {
    font-weight: 500;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
