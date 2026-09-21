<script lang="ts">
  import { api, type Album, type Playlist, type Track } from "../lib/api";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  let { nav, player }: { nav: Nav; player: Player } = $props();
  let tracks = $state<Track[]>([]);
  let albums = $state<Album[]>([]);
  let playlists = $state<Playlist[]>([]);
  let loading = $state(true);
  let error = $state("");

  $effect(() => {
    loading = true;
    error = "";
    Promise.all([api.getFavoriteTracks(), api.getFavoriteAlbums(), api.getUserPlaylists()])
      .then(([t, a, p]) => {
        tracks = t;
        albums = a;
        playlists = p;
      })
      .catch((e) => {
        error = String(e);
      })
      .finally(() => {
        loading = false;
      });
  });
</script>

<h2>Library</h2>
{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <p style="color:#ff6b6b">{error}</p>
{:else}
  <h3>Playlists</h3>
  <ul>
    {#each playlists as p (p.uuid)}
      <li><button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button></li>
    {/each}
    {#if playlists.length === 0}<li class="muted">No playlists</li>{/if}
  </ul>
  <h3>Albums</h3>
  <ul>
    {#each albums as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button></li>
    {/each}
    {#if albums.length === 0}<li class="muted">No albums</li>{/if}
  </ul>
  <h3>Favorite tracks</h3>
  <TrackTable {tracks} {player} />
{/if}
