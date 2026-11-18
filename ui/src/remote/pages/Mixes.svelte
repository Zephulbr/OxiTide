<script lang="ts">
  import Loader from "../components/Loader.svelte";
  import TileGrid from "../components/TileGrid.svelte";
  import { useRemote } from "../context";
  import { mixTile } from "../convert";
  import { t } from "../i18n.svelte";

  const { api } = useRemote();
</script>

<div class="page">
  <header class="page-head"><h1>{t("library.mixes")}</h1></header>
  <Loader load={api.getFavoriteMixes}>
    {#snippet children(mixes)}
      {#if mixes.length === 0}
        <p class="muted">{t("mixes.empty")}</p>
      {:else}
        <TileGrid tiles={mixes.map(mixTile)} label={t("filter.mixes")} />
      {/if}
    {/snippet}
  </Loader>
</div>
