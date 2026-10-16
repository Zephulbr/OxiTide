<script lang="ts">
  import { untrack } from "svelte";
  import type { Mix, PageItem, PageSection } from "../lib/api";
  import { load as loadCached, peek } from "../lib/viewCache";
  import { favoriteMixesSource, forYouSource } from "../lib/viewData";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let { nav, player, toasts }: { nav: Nav; player: Player; toasts: Toasts } = $props();

  // Cached visits render complete on the first frame; content that had to
  // load rises in (`.arrive`) instead of snapping in. Both parts show
  // together, so neither pushes the other down when it lands.
  const sources = { forYou: forYouSource(), mixes: favoriteMixesSource() };
  const cachedForYou = peek(sources.forYou);
  const cachedMixes = peek(sources.mixes);
  const arrive = cachedForYou === undefined || cachedMixes === undefined;
  let forYou = $state<PageSection[]>(cachedForYou ?? []);
  let mixes = $state<Mix[]>(cachedMixes ?? []);
  let loading = $state(arrive);
  let error = $state("");

  async function load() {
    if (peek(sources.forYou) === undefined || peek(sources.mixes) === undefined) loading = true;
    error = "";
    const [, mixesResult] = await Promise.allSettled([
      loadCached(sources.forYou, (sections) => (forYou = sections)).catch(() => (forYou = [])),
      loadCached(sources.mixes, (list) => (mixes = list)),
    ]);
    if (mixesResult.status === "rejected") error = String(mixesResult.reason);
    loading = false;
  }

  $effect(() => {
    untrack(() => void load());
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
  <div class:arrive>
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
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
  }
</style>
