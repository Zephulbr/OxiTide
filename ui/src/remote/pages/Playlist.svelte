<script lang="ts">
  import DetailHead from "../components/DetailHead.svelte";
  import Loader from "../components/Loader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import { useRemote } from "../context";
  import { art } from "../convert";
  import { PagedTracks } from "../paged.svelte";

  let { id }: { id: string } = $props();
  const { api } = useRemote();

  async function load() {
    const [playlist, paged] = await Promise.all([
      api.getPlaylist(id),
      new PagedTracks((offset, limit) => api.getPlaylistTracksPage(id, offset, limit)).first(),
    ]);
    return { playlist, paged };
  }
</script>

<div class="page">
  <Loader {load} key={id}>
    {#snippet children({ playlist, paged })}
      <DetailHead
        image={art(playlist.squareImage ?? playlist.image, 640)}
        title={playlist.title}
        kind="Playlist"
        subtitle={`${playlist.numberOfTracks} tracks`}
      />
      {#if playlist.description}<p class="muted desc">{playlist.description}</p>{/if}
      <TrackList tracks={paged.rows} total={paged.total} more={paged.more} />
    {/snippet}
  </Loader>
</div>

<style>
  .desc {
    margin: -4px 0 16px;
    text-align: center;
    font-size: 13px;
  }
</style>
