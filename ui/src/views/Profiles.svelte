<script lang="ts">
  import { api, type Artist } from "../lib/api";
  import { groupArtists, visibleArtists, type ArtistSort } from "../lib/artistSort";
  import Cover from "../lib/components/Cover.svelte";
  import EdgeBlur from "../lib/components/EdgeBlur.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import SortSelect from "../lib/components/SortSelect.svelte";
  import type { Nav } from "../lib/nav.svelte";

  let { nav }: { nav: Nav } = $props();

  let artists = $state<Artist[]>([]);
  let loading = $state(true);
  let error = $state("");
  let query = $state("");
  let sortMode = $state<ArtistSort>("date");
  let headEl = $state<HTMLElement | null>(null);
  let scrolled = $state(false);

  const shown = $derived(visibleArtists(artists, query, sortMode));
  const groups = $derived(sortMode === "date" ? null : groupArtists(shown));

  async function load() {
    loading = true;
    error = "";
    try {
      artists = await api.getFavoriteArtists();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void load();
  });

  $effect(() => {
    const main = headEl?.closest(".main");
    if (!main) return;
    const sync = () => (scrolled = main.scrollTop > 4);
    sync();
    main.addEventListener("scroll", sync, { passive: true });
    return () => main.removeEventListener("scroll", sync);
  });
</script>

{#snippet cards(list: Artist[])}
  {#each list as artist (artist.id)}
    <button class="card" onclick={() => nav.goto({ name: "artist", id: artist.id })}>
      <span class="art"><Cover uuid={artist.picture} size={320} radius={160} alt="" /></span>
      <span class="title">{artist.name}</span>
    </button>
  {/each}
{/snippet}

<div class="profiles">
  <div class="page-head" class:scrolled bind:this={headEl}>
    <div class="head-frost" aria-hidden="true">
      <div class="head-ramp">
        <EdgeBlur layers={10} height={160} tint="var(--glass-veil)" tintStrength={75} />
      </div>
    </div>
    <h2>Profiles</h2>
    <span class="muted count">{artists.length} {artists.length === 1 ? "artist" : "artists"}</span>
    <input
      class="search-input filter"
      placeholder="Search artists"
      value={query}
      oninput={(event) => (query = event.currentTarget.value)}
      aria-label="Search artists"
    />
    <SortSelect
      value={sortMode}
      options={[
        { value: "date", label: "Date added" },
        { value: "name", label: "Name A–Z" },
        { value: "name-desc", label: "Name Z–A" },
      ]}
      label="Sort artists"
      onchange={(value) => (sortMode = value as ArtistSort)}
    />
  </div>

  {#if loading}
    <Loading />
  {:else if error}
    <ErrorState message={error} onRetry={load} />
  {:else if artists.length === 0}
    <EmptyState message="No artists yet." />
  {:else if shown.length === 0}
    <p class="muted empty">No artists match “{query.trim()}”.</p>
  {:else if groups}
    <div class="groups">
      {#each groups as group (group.letter)}
        <h3 class="letter">{group.letter}</h3>
        <div class="grid">{@render cards(group.artists)}</div>
      {/each}
    </div>
  {:else}
    <div class="grid">{@render cards(shown)}</div>
  {/if}
</div>

<style>
  .profiles {
    padding: 32px 48px 0;
  }
  .page-head {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0 -48px;
    padding: 40px 48px 0;
  }
  .head-frost {
    position: absolute;
    inset: 0;
    z-index: -1;
    pointer-events: none;
  }
  .head-ramp {
    position: absolute;
    top: 0;
    right: 0;
    left: 0;
    visibility: hidden;
  }
  .page-head.scrolled .head-ramp {
    visibility: visible;
  }
  .page-head h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
  }
  .count {
    font-size: 13px;
  }
  .filter {
    flex: 0 1 240px;
  }
  .empty {
    margin-top: 72px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 56px 16px;
    margin-top: 72px;
  }
  .groups {
    margin-top: 72px;
  }
  .groups .grid {
    margin-top: 12px;
    padding: 0;
  }
  .letter {
    margin: 48px 0 0;
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .groups .letter:first-child {
    margin-top: 0;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    text-align: center;
    padding: 4px;
    border-radius: var(--radius-card);
    color: inherit;
  }
  .card:hover {
    background: var(--surface-2);
  }
  .art {
    display: block;
    width: 100%;
    overflow: hidden;
    border-radius: 50%;
  }
  .title {
    font-size: 15px;
    font-weight: 500;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
