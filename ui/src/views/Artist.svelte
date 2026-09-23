<script lang="ts">
  import { api, type Artist as ArtistT } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  let { id }: { id: number } = $props();
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
  <p class="muted">Loading…</p>
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if artist}
  <h2>{artist.name}</h2>
  <p class="muted">Artist</p>
{/if}
