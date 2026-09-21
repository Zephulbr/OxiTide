<script lang="ts">
  import { api, type Album as AlbumT, type Track } from "../lib/api";
  import TrackTable from "../lib/components/TrackTable.svelte";
  let { id }: { id: number } = $props();
  let album = $state<AlbumT | null>(null);
  let tracks = $state<Track[]>([]);

  $effect(() => {
    api.getAlbum(id).then((a) => (album = a));
    api.getAlbumTracks(id).then((t) => (tracks = t));
  });
</script>

{#if album}
  <h2>{album.title}</h2>
  <p class="muted">{album.artists.map((a) => a.name).join(", ")} · {album.numberOfTracks} tracks</p>
{/if}
<TrackTable {tracks} />
