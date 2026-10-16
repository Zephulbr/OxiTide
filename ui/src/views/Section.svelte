<script lang="ts">
  import { api, type PageSection } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import type { Nav, SectionSource } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let {
    source,
    title,
    nav,
    player,
    toasts,
  }: {
    source: SectionSource;
    title: string;
    nav: Nav;
    player: Player;
    toasts: Toasts;
  } = $props();

  let section = $state<PageSection | null>(null);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      const sections =
        source === "home"
          ? await api.getHome()
          : source === "explore"
            ? await api.getExplore()
            : source === "forYou"
              ? await api.getForYou()
              : await api.getPage(source);
      section = sections.find((s) => s.title === title) ?? null;
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

<h2>{title}</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if !section}
  <EmptyState message="Section no longer available." />
{:else}
  <p class="muted count">{section.items.length} items</p>
  <PageSections sections={[section]} {nav} {player} {toasts} {source} uncapped showTitle={false} />
{/if}

<style>
  .count {
    margin-top: -8px;
  }
</style>
