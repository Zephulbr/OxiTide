<script lang="ts">
  import Loader from "../components/Loader.svelte";
  import Shelf from "../components/Shelf.svelte";
  import TrackList from "../components/TrackList.svelte";
  import { useRemote } from "../context";
  import { albumTile, artistTile } from "../convert";
  import { PagedTracks } from "../paged.svelte";
  import { t } from "../i18n.svelte";

  let { kind }: { kind: "tracks" | "albums" | "artists" } = $props();
  const { api } = useRemote();

  const TITLES = { tracks: "library.tracks", albums: "library.albums", artists: "library.artists" } as const;
</script>

<div class="page">
  <header class="page-head"><h1>{t(TITLES[kind])}</h1></header>
  {#if kind === "tracks"}
    <Loader load={() => new PagedTracks(api.getFavoriteTracksPage).first()} key={kind}>
      {#snippet children(paged)}
        {#if paged.total === 0}
          <p class="muted">{t("favorites.noTracks")}</p>
        {:else}
          <TrackList tracks={paged.rows} total={paged.total} more={paged.more} filterable />
        {/if}
      {/snippet}
    </Loader>
  {:else if kind === "albums"}
    <Loader load={api.getFavoriteAlbums} key={kind}>
      {#snippet children(albums)}
        {#if albums.length === 0}<p class="muted">{t("favorites.noAlbums")}</p>{/if}
        <Shelf tiles={albums.map(albumTile)} grid />
      {/snippet}
    </Loader>
  {:else}
    <Loader load={api.getFavoriteArtists} key={kind}>
      {#snippet children(artists)}
        {#if artists.length === 0}<p class="muted">{t("favorites.noArtists")}</p>{/if}
        <Shelf tiles={artists.map(artistTile)} grid />
      {/snippet}
    </Loader>
  {/if}
</div>
