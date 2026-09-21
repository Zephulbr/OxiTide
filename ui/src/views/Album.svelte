<script lang="ts">
  import { api, type Album as AlbumT, type Track } from "../lib/api";
  import TrackTable from "../lib/components/TrackTable.svelte";
  let { id }: { id: number } = $props();
  let album = $state<AlbumT | null>(null);
  let tracks = $state<Track[]>([]);
  let loading = $state(true);
  let error = $state("");

  $effect(() => {
    loading = true;
    error = "";
    Promise.all([api.getAlbum(id), api.getAlbumTracks(id)])
      .then(([a, t]) => {
        album = a;
        tracks = t;
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
{:else}
  {#if album}
    <h2>{album.title}</h2>
    <p class="muted">{album.artists.map((a) => a.name).join(", ")} · {album.numberOfTracks} tracks</p>
  {/if}
  <TrackTable {tracks} />
{/if}
