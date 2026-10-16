<script lang="ts">
  import { api, type PageSection } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import type { Nav, PagePath } from "../lib/nav.svelte";
  import { withoutVideoLinks } from "../lib/pageLinks";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  /// A TIDAL page a page link opens (a genre, mood or decade from Explore).
  /// Same rows/modules shape as Explore, so it renders through PageSections;
  /// its own page links (sub-genres) open further pages the same way.
  let {
    path,
    title,
    nav,
    player,
    toasts,
  }: {
    path: PagePath;
    title: string;
    nav: Nav;
    player: Player;
    toasts: Toasts;
  } = $props();

  let sections = $state<PageSection[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      sections = withoutVideoLinks(await api.getPage(path)).filter((section) => section.items.length > 0);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void load();
  });
</script>

<header class="page-head title">
  <button type="button" class="eyebrow" onclick={() => nav.goto({ name: "explore" })}>Explore</button>
  <h2>{title}</h2>
</header>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if sections.length === 0}
  <EmptyState message="Nothing here yet." />
{:else}
  <PageSections {sections} {nav} {player} {toasts} source={path} />
{/if}

<style>
  .title {
    flex-direction: column;
    flex-wrap: nowrap;
    align-items: flex-start;
    gap: 2px;
  }
  .eyebrow {
    padding: 0;
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .eyebrow:hover {
    color: var(--text);
  }
</style>
