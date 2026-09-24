<script lang="ts">
  import { api, type Album as AlbumT, type Track } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  let {
    id,
    player,
    favorites,
    playlistActions,
  }: { id: number; player: Player; favorites: Favorites; playlistActions: PlaylistActions } = $props();
  let album = $state<AlbumT | null>(null);
  let tracks = $state<Track[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      [album, tracks] = await Promise.all([api.getAlbum(id), api.getAlbumTracks(id)]);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });
</script>

{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else}
  {#if album}
    {@const albumId = album.id}
    <h2>
      {album.title}
      <button
        class:on={favorites.hasAlbum(albumId)}
        onclick={() => favorites.toggleAlbum(albumId)}
        aria-label={favorites.hasAlbum(albumId) ? "Unfavorite album" : "Favorite album"}
        title={favorites.hasAlbum(albumId) ? "Unfavorite album" : "Favorite album"}
      >{favorites.hasAlbum(albumId) ? "♥" : "♡"}</button>
      <button
        onclick={() => playlistActions.openPicker(tracks)}
        aria-label="Add album to playlist"
        title="Add album to playlist"
      >Add to playlist</button>
    </h2>
    <p class="muted">{album.artists.map((a) => a.name).join(", ")} · {album.numberOfTracks} tracks</p>
  {/if}
  <TrackTable
    {tracks}
    {player}
    {favorites}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
  />
{/if}
