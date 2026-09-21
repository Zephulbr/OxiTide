<script lang="ts">
  import { api, type Playlist as PlaylistT, type Track } from "../lib/api";
  import TrackTable from "../lib/components/TrackTable.svelte";
  let { id }: { id: string } = $props();
  let playlist = $state<PlaylistT | null>(null);
  let tracks = $state<Track[]>([]);

  $effect(() => {
    api.getPlaylist(id).then((p) => (playlist = p));
    api.getPlaylistTracks(id).then((t) => (tracks = t));
  });
</script>

{#if playlist}
  <h2>{playlist.title}</h2>
  <p class="muted">{playlist.number_of_tracks} tracks</p>
{/if}
<TrackTable {tracks} />
