<script lang="ts">
  import Loader from "../components/Loader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import { useRemote } from "../context";
  import { fromHistory } from "../convert";
  import { PagedTracks } from "../paged.svelte";
  import { t } from "../i18n.svelte";

  const { api } = useRemote();
</script>

<div class="page">
  <header class="page-head"><h1>{t("library.recent")}</h1></header>
  <!-- No dedupe: a song played twice is two plays. -->
  <Loader load={() => new PagedTracks(api.getHistoryPage, fromHistory).first()}>
    {#snippet children(paged)}
      {#if paged.total === 0}
        <p class="muted">{t("history.empty")}</p>
      {:else}
        <TrackList tracks={paged.rows} paging={paged} filterable />
      {/if}
    {/snippet}
  </Loader>
</div>
