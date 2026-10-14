<script lang="ts">
  import Loader from "../components/Loader.svelte";
  import Shelf from "../components/Shelf.svelte";
  import { useRemote } from "../context";
  import { mixTile } from "../convert";

  const { api } = useRemote();
</script>

<div class="page">
  <header class="page-head"><h1>My Mixes</h1></header>
  <Loader load={api.getFavoriteMixes}>
    {#snippet children(mixes)}
      {#if mixes.length === 0}
        <p class="muted">No saved mixes yet.</p>
      {:else}
        <Shelf tiles={mixes.map(mixTile)} grid />
      {/if}
    {/snippet}
  </Loader>
</div>
