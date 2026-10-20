<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import DetailHead from "../components/DetailHead.svelte";
  import Loader from "../components/Loader.svelte";
  import Shelf from "../components/Shelf.svelte";
  import TrackRow from "../components/TrackRow.svelte";
  import { useRemote } from "../context";
  import { albumTile, artistImage, artistTile, fromTrack } from "../convert";

  let { id }: { id: number } = $props();
  const { api, actions, store } = useRemote();
  const playingId = $derived(store.state?.current?.trackId ?? null);

  async function load() {
    void actions.ensureFavorites();
    const [artist, top, albums, singles, similar] = await Promise.all([
      api.getArtist(id),
      api.getArtistTopTracksPage(id, 0, 10),
      api.getArtistAlbumsAll(id, "albums"),
      api.getArtistAlbumsAll(id, "eps_and_singles").catch(() => []),
      api.getSimilarArtists(id).catch(() => []),
    ]);
    return { artist, top: top.items.map(fromTrack), albums, singles, similar };
  }

  async function radio() {
    try {
      const tracks = await api.getArtistRadio(id);
      await actions.play(tracks.map(fromTrack));
    } catch (e) {
      actions.fail(e);
    }
  }
</script>

<div class="page">
  <Loader {load} key={id}>
    {#snippet children({ artist, top, albums, singles, similar })}
      <DetailHead
        image={artistImage(artist)}
        title={artist.name}
        kind="Artist"
        round
        favorite={actions.favArtists.has(artist.id)}
        ontogglefavorite={() => actions.toggleArtist(artist.id)}
      >
        <button class="pill primary" disabled={top.length === 0} onclick={() => actions.play(top, 0)}>
          <Icon name="play" size={18} /> Play
        </button>
        <button class="pill" onclick={radio}><Icon name="radio" size={18} /> Radio</button>
      </DetailHead>
      {#if top.length > 0}
        <h2 class="section-title">Top tracks</h2>
        {#each top as row, i (row.id)}
          <TrackRow
            track={row}
            index={i}
            active={row.id === playingId}
            onplay={() => actions.play(top, i)}
            onmenu={() => actions.openTrack(row)}
          />
        {/each}
      {/if}
      {#if albums.length > 0}<Shelf title="Albums" tiles={albums.map(albumTile)} />{/if}
      {#if singles.length > 0}<Shelf title="EPs & singles" tiles={singles.map(albumTile)} />{/if}
      {#if similar.length > 0}<Shelf title="Fans also like" tiles={similar.map(artistTile)} />{/if}
    {/snippet}
  </Loader>
</div>
