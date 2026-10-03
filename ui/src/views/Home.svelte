<script lang="ts">
  import { api, type PageSection } from "../lib/api";
  import { shortcutEntries, type ShortcutEntry } from "../lib/media";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let { nav, toasts }: { nav: Nav; toasts: Toasts } = $props();

  let sections = $state<PageSection[]>([]);
  let shortcuts = $state<ShortcutEntry[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      sections = await api.getHome();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function loadShortcuts() {
    const [playlists, mixes] = await Promise.allSettled([
      api.getUserPlaylists(),
      api.getFavoriteMixes(),
    ]);
    shortcuts = shortcutEntries(
      playlists.status === "fulfilled" ? playlists.value : [],
      mixes.status === "fulfilled" ? mixes.value : [],
    );
  }

  $effect(() => {
    void load();
    void loadShortcuts();
  });
</script>

<h2>Home</h2>
{#if shortcuts.length > 0}
  <section class="shortcuts" aria-label="Quick access">
    {#each shortcuts as entry (entry.key)}
      <MediaCard
        item={entry.item}
        variant="row"
        thumb={64}
        collage={entry.playlist}
        onclick={() => nav.goto(entry.route)}
      />
    {/each}
  </section>
{/if}
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if sections.length === 0}
  <EmptyState message="Nothing to show yet." />
{:else}
  <PageSections {sections} {nav} {toasts} source="home" />
{/if}

<style>
  .shortcuts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(330px, 1fr));
    gap: 12px 20px;
    margin-bottom: 24px;
  }
</style>
