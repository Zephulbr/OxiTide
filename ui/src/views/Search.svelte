<script lang="ts">
  import { api, type SearchResults } from "../lib/api";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import type { Nav } from "../lib/nav.svelte";
  let { nav }: { nav: Nav } = $props();
  let query = $state("");
  let results = $state<SearchResults | null>(null);

  async function run() {
    if (query.trim().length === 0) return;
    results = await api.search(query);
  }
</script>

<h2>Search</h2>
<input bind:value={query} onkeydown={(e) => e.key === "Enter" && run()} placeholder="Search TIDAL" />
{#if results}
  <h3>Tracks</h3>
  <TrackTable tracks={results.tracks.items} />
  <h3>Albums</h3>
  <ul>
    {#each results.albums.items as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button></li>
    {/each}
  </ul>
  <h3>Artists</h3>
  <ul>
    {#each results.artists.items as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "artist", id: a.id })}>{a.name}</button></li>
    {/each}
  </ul>
  <h3>Playlists</h3>
  <ul>
    {#each results.playlists.items as p (p.uuid)}
      <li><button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button></li>
    {/each}
  </ul>
{/if}
