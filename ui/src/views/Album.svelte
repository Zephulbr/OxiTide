<script lang="ts">
  import { api, type Album as AlbumT, type Track } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Player } from "../lib/player.svelte";
  let { id, player, favorites }: { id: number; player: Player; favorites: Favorites } = $props();
  let album = $state<AlbumT | null>(null);
  let tracks = $state<Track[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      [album, tracks] = await Promise.all([api.getAlbum(id), api.getAlbumTracks(id)]);
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
{:else}
  {#if album}
    <h2>{album.title}</h2>
    <p class="muted">{album.artists.map((a) => a.name).join(", ")} · {album.numberOfTracks} tracks</p>
  {/if}
  <TrackTable {tracks} {player} {favorites} />
{/if}
