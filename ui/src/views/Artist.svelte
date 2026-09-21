<script lang="ts">
  import { api, type Artist as ArtistT } from "../lib/api";
  let { id }: { id: number } = $props();
  let artist = $state<ArtistT | null>(null);
  $effect(() => { api.getArtist(id).then((a) => (artist = a)); });
</script>

{#if artist}
  <h2>{artist.name}</h2>
  <p class="muted">Artist</p>
{/if}
