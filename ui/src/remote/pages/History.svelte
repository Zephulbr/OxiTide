<script lang="ts">
  import type { HistoryEntry } from "../../lib/api";
  import Loader from "../components/Loader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import { useRemote } from "../context";
  import { fromHistory } from "../convert";

  const { api } = useRemote();
  const PAGE = 100;

  let entries = $state<HistoryEntry[]>([]);
  let total = $state(0);

  async function first() {
    const page = await api.getHistoryPage(0, PAGE);
    entries = page.items;
    total = page.total;
    return page;
  }

  async function more() {
    const page = await api.getHistoryPage(entries.length, PAGE);
    entries = [...entries, ...page.items];
    total = page.items.length === 0 ? entries.length : page.total;
  }

  const rows = $derived(entries.map(fromHistory));
</script>

<div class="page">
  <header class="page-head"><h1>Recently played</h1></header>
  <Loader load={first}>
    {#snippet children()}
      {#if total === 0}
        <p class="muted">Nothing played yet (or history is off on the PC).</p>
      {:else}
        <TrackList tracks={rows} {total} {more} />
      {/if}
    {/snippet}
  </Loader>
</div>
