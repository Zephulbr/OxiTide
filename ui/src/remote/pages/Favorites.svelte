<script lang="ts">
  import Loader from "../components/Loader.svelte";
  import Shelf from "../components/Shelf.svelte";
  import TrackList from "../components/TrackList.svelte";
  import { useRemote } from "../context";
  import { albumTile, artistTile } from "../convert";
  import { PagedTracks } from "../paged.svelte";

  let { kind }: { kind: "tracks" | "albums" | "artists" } = $props();
  const { api } = useRemote();

  const TITLES = { tracks: "Tracks", albums: "Albums", artists: "Artists" } as const;
</script>

<div class="page">
  <header class="page-head"><h1>{TITLES[kind]}</h1></header>
  {#if kind === "tracks"}
    <Loader load={() => new PagedTracks(api.getFavoriteTracksPage).first()} key={kind}>
      {#snippet children(paged)}
        {#if paged.total === 0}
          <p class="muted">No favorite tracks yet.</p>
        {:else}
          <TrackList tracks={paged.rows} total={paged.total} more={paged.more} />
        {/if}
      {/snippet}
    </Loader>
  {:else if kind === "albums"}
    <Loader load={api.getFavoriteAlbums} key={kind}>
      {#snippet children(albums)}
        {#if albums.length === 0}<p class="muted">No favorite albums yet.</p>{/if}
        <Shelf tiles={albums.map(albumTile)} grid />
      {/snippet}
    </Loader>
  {:else}
    <Loader load={api.getFavoriteArtists} key={kind}>
      {#snippet children(artists)}
        {#if artists.length === 0}<p class="muted">No favorite artists yet.</p>{/if}
        <Shelf tiles={artists.map(artistTile)} grid />
      {/snippet}
    </Loader>
  {/if}
</div>
