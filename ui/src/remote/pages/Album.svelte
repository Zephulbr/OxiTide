<script lang="ts">
  import { formatTotalDuration } from "../../lib/format";
  import DetailHead from "../components/DetailHead.svelte";
  import Loader from "../components/Loader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import { useRemote } from "../context";
  import { art, fromTrack } from "../convert";
  import { hrefFor } from "../routes";

  let { id }: { id: number } = $props();
  const { api, actions } = useRemote();

  async function load() {
    void actions.ensureFavorites();
    const [album, tracks] = await Promise.all([api.getAlbum(id), api.getAlbumTracks(id)]);
    return { album, rows: tracks.map(fromTrack) };
  }
</script>

<div class="page">
  <Loader {load} key={id}>
    {#snippet children({ album, rows })}
      <DetailHead
        image={art(album.cover, 640)}
        title={album.title}
        kind="Album"
        subtitle={[
          album.releaseDate?.slice(0, 4),
          `${album.numberOfTracks} tracks`,
          formatTotalDuration(album.duration),
        ]
          .filter(Boolean)
          .join(" · ")}
        favorite={actions.favAlbums.has(album.id)}
        ontogglefavorite={() => actions.toggleAlbum(album.id)}
      />
      <p class="artists">
        {#each album.artists as artist, i (artist.id)}
          {#if i > 0}<span class="muted">, </span>{/if}
          <a href={hrefFor({ name: "artist", id: artist.id })}>{artist.name}</a>
        {/each}
      </p>
      <TrackList tracks={rows} numbered />
    {/snippet}
  </Loader>
</div>

<style>
  .artists {
    margin: -4px 0 16px;
    text-align: center;
    font-weight: 600;
    color: var(--accent);
  }
</style>
