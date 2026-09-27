<script lang="ts">
  import { api, type Artist as ArtistT } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  let { id, favorites }: { id: number; favorites: Favorites } = $props();
  let artist = $state<ArtistT | null>(null);
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
          class:on={favorites.hasArtist(artistId)}
          onclick={() => favorites.toggleArtist(artistId)}
          aria-label={favorites.hasArtist(artistId) ? "Unfavorite artist" : "Favorite artist"}
          title={favorites.hasArtist(artistId) ? "Unfavorite artist" : "Favorite artist"}
        ><Icon name="heart" /></button>
      </div>
    </div>
  </header>
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
</style>
