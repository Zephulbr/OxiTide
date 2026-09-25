<script lang="ts">
  import { api, type Artist as ArtistT } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
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
  <h2>
    {artist.name}
    <button
      class:on={favorites.hasArtist(artistId)}
      onclick={() => favorites.toggleArtist(artistId)}
      aria-label={favorites.hasArtist(artistId) ? "Unfavorite artist" : "Favorite artist"}
      title={favorites.hasArtist(artistId) ? "Unfavorite artist" : "Favorite artist"}
    >{favorites.hasArtist(artistId) ? "♥" : "♡"}</button>
  </h2>
  <p class="muted">Artist</p>
{:else}
  <EmptyState message="Artist not found." />
{/if}
