<script lang="ts">
  import { api, type Playlist as PlaylistT, type Track } from "../lib/api";
  import TrackTable from "../lib/components/TrackTable.svelte";
  let { id }: { id: string } = $props();
  let playlist = $state<PlaylistT | null>(null);
  let tracks = $state<Track[]>([]);
  let loading = $state(true);
  let error = $state("");

  $effect(() => {
    loading = true;
    error = "";
    Promise.all([api.getPlaylist(id), api.getPlaylistTracks(id)])
      .then(([p, t]) => {
        playlist = p;
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
  {#if playlist}
    <h2>{playlist.title}</h2>
    <p class="muted">{playlist.numberOfTracks} tracks</p>
  {/if}
  <TrackTable {tracks} />
{/if}
