<script lang="ts">
  import { api, type Mix, type PageItem, type PageSection } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let { nav, player, toasts }: { nav: Nav; player: Player; toasts: Toasts } = $props();

  let forYou = $state<PageSection[]>([]);
  let mixes = $state<Mix[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    const [forYouResult, mixesResult] = await Promise.allSettled([
      api.getForYou(),
      api.getFavoriteMixes(),
    ]);
    forYou = forYouResult.status === "fulfilled" ? forYouResult.value : [];
    if (mixesResult.status === "fulfilled") {
      mixes = mixesResult.value;
    } else {
      error = String(mixesResult.reason);
    }
    loading = false;
  }

  $effect(() => {
    void load();
  });
</script>

<h2>Mixes &amp; Radio</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if forYou.length === 0 && mixes.length === 0}
  <EmptyState message="No mixes yet." />
{:else}
  {#if forYou.length > 0}
    <PageSections sections={forYou} {nav} {player} {toasts} source="forYou" />
  {/if}
  {#if mixes.length > 0}
    <h3>Favorite mixes</h3>
    <div class="grid">
      {#each mixes as mix (mix.id)}
        {@const item: PageItem = {
          id: mix.id,
          title: mix.title,
          subtitle: mix.subTitle ?? null,
          item_type: "MIX_LIST",
          artwork: mix.image ?? null,
        }}
        <MediaCard {item} variant="tile" onclick={() => nav.goto({ name: "mix", id: mix.id })} />
      {/each}
    </div>
  {/if}
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
  }
</style>
