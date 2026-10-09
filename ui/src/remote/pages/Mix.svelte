<script lang="ts">
  import DetailHead from "../components/DetailHead.svelte";
  import Loader from "../components/Loader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import { useRemote } from "../context";
  import { art, fromTrack } from "../convert";

  let { id }: { id: string } = $props();
  const { api } = useRemote();

  async function load() {
    const mix = await api.getMix(id);
    return { mix, rows: mix.tracks.map(fromTrack) };
  }
</script>

<div class="page">
  <Loader {load} key={id}>
    {#snippet children({ mix, rows })}
      <DetailHead image={art(mix.artwork, 640)} title={mix.title} kind="Mix" subtitle={`${rows.length} tracks`} />
      <TrackList tracks={rows} />
    {/snippet}
  </Loader>
</div>
