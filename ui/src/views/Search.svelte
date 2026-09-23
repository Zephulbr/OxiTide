<script lang="ts">
  import { api, type SearchResults } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  let { nav, player }: { nav: Nav; player: Player } = $props();
  let query = $state("");
  let results = $state<SearchResults | null>(null);
  let loading = $state(false);
  let error = $state("");

  async function load() {
    if (query.trim().length === 0) return;
    loading = true;
    error = "";
    try {
      results = await api.search(query);
    } catch (e) {
      error = String(e);
      results = null;
    } finally {
      loading = false;
    }
  }
</script>

<h2>Search</h2>
<input bind:value={query} onkeydown={(e) => e.key === "Enter" && load()} placeholder="Search TIDAL" />
{#if loading}
  <p class="muted">Searching…</p>
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if results}
  <h3>Tracks</h3>
  <TrackTable tracks={results.tracks.items} {player} />
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
