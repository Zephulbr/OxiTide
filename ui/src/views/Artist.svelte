<script lang="ts">
  import { api, type Artist as ArtistT } from "../lib/api";
  let { id }: { id: number } = $props();
  let artist = $state<ArtistT | null>(null);
  let loading = $state(true);
  let error = $state("");

  $effect(() => {
    loading = true;
    error = "";
    api
      .getArtist(id)
      .then((a) => {
        artist = a;
      })
      .catch((e) => {
        error = String(e);
      })
      .finally(() => {
        loading = false;
      });
  });
</script>

{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <p style="color:#ff6b6b">{error}</p>
{:else if artist}
  <h2>{artist.name}</h2>
  <p class="muted">Artist</p>
{/if}
