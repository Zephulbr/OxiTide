<script lang="ts">
  import { api, type Album, type Playlist, type Track } from "../lib/api";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import type { Nav } from "../lib/nav.svelte";
  let { nav }: { nav: Nav } = $props();
  let tracks = $state<Track[]>([]);
  let albums = $state<Album[]>([]);
  let playlists = $state<Playlist[]>([]);

  $effect(() => {
    api.getFavoriteTracks().then((t) => (tracks = t));
    api.getFavoriteAlbums().then((a) => (albums = a));
    api.getUserPlaylists().then((p) => (playlists = p));
  });
</script>

<h2>Library</h2>
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
<TrackTable {tracks} />
