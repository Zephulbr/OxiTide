<script lang="ts">
  import { untrack } from "svelte";
  import type { Artist } from "../lib/api";
  import { load as loadCached, peek } from "../lib/viewCache";
  import { favoriteArtistsSource } from "../lib/viewData";
  import { groupArtists, visibleArtists, type ArtistSort } from "../lib/artistSort";
  import Cover from "../lib/components/Cover.svelte";
  import EdgeBlur from "../lib/components/EdgeBlur.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import SortSelect from "../lib/components/SortSelect.svelte";
  import type { Nav } from "../lib/nav.svelte";

  let { nav }: { nav: Nav } = $props();

  // Cached visits render complete on the first frame; a list that had to load
  // rises in (`.arrive`) with its count instead of snapping in.
  const source = favoriteArtistsSource();
  const cached = peek(source);
  const arrive = cached === undefined;
  let artists = $state<Artist[]>(cached ?? []);
  let loading = $state(cached === undefined);
  let error = $state("");
  let query = $state("");
  let sortMode = $state<ArtistSort>("date");
  let headEl = $state<HTMLElement | null>(null);
  let scrolled = $state(false);

  const shown = $derived(visibleArtists(artists, query, sortMode));
  const groups = $derived(sortMode === "date" ? null : groupArtists(shown));

  async function load() {
    if (peek(source) === undefined) loading = true;
    error = "";
    try {
      await loadCached(source, (list) => (artists = list));
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    untrack(() => void load());
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

<!-- Each artist is a record: the portrait is the label on a vinyl disc. In
     name order the letters are crate dividers. -->
{#snippet records(list: Artist[])}
  {#each list as artist (artist.id)}
    <button class="card" onclick={() => nav.goto({ name: "artist", id: artist.id })}>
      <span class="record" aria-hidden="true">
        <span class="label"><Cover uuid={artist.picture} size={320} radius={160} alt="" /></span>
      </span>
      <span class="title">{artist.name}</span>
    </button>
  {/each}
{/snippet}

<div class="profiles">
  <header class="page-head" class:scrolled bind:this={headEl}>
    <div class="head-frost" aria-hidden="true">
      <div class="head-ramp">
        <EdgeBlur layers={10} height={140} strength={1.3} tint="var(--glass-veil)" tintStrength={75} />
      </div>
    </div>
    <h2>Profiles</h2>
    <!-- Space held while loading (no "0 artists" flash, no header reflow). -->
    <span
      class="muted count tnum"
      class:arrive={arrive && !loading}
      style:visibility={loading || error ? "hidden" : undefined}
    >{artists.length} {artists.length === 1 ? "artist" : "artists"}</span>
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
  </header>

  {#if loading}
    <Loading />
  {:else if error}
    <ErrorState message={error} onRetry={load} />
  {:else if artists.length === 0}
    <EmptyState message="No artists yet." />
  {:else if shown.length === 0}
    <p class="muted empty">No artists match “{query.trim()}”.</p>
  {:else if groups}
    {#each groups as group (group.letter)}
      <h3 class="divider" class:arrive>
        <span class="tab">{group.letter}</span>
        <span class="muted n tnum">{group.artists.length}</span>
      </h3>
      <div class="grid pane" class:arrive>{@render records(group.artists)}</div>
    {/each}
  {:else}
    <div class="grid pane" class:arrive>{@render records(shown)}</div>
  {/if}
</div>

<style>
  .profiles {
    padding-bottom: 32px;
  }
  /* The shared page head, made sticky with a frost ramp once the page scrolls
     (the main scroller's top padding is dropped for this view, app.css). */
  .page-head {
    position: sticky;
    top: 0;
    z-index: 5;
    margin: 0 0 8px;
    padding: 32px 0 16px;
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
    right: -12px;
    left: -12px;
    visibility: hidden;
  }
  .page-head.scrolled .head-ramp {
    visibility: visible;
  }
  .filter {
    flex: 0 1 240px;
    min-width: 140px;
  }
  .empty {
    margin-top: 48px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
    gap: 28px 20px;
    margin-top: 16px;
  }

  /* Crate dividers: a tab with the letter standing up from a rule. */
  .divider {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    margin: 36px 0 0;
    border-bottom: 1px solid var(--line);
  }
  .divider:first-of-type {
    margin-top: 8px;
  }
  .tab {
    min-width: 40px;
    margin-bottom: -1px;
    padding: 6px 12px 5px;
    border: 1px solid var(--line);
    border-bottom-color: transparent;
    border-radius: var(--radius-control) var(--radius-control) 0 0;
    background: linear-gradient(var(--surface-3), var(--surface-1));
    font-family: var(--font-display);
    font-size: 18px;
    font-weight: 640;
    text-align: center;
  }
  .n {
    padding-bottom: 7px;
    font-size: 12px;
    font-weight: 500;
  }

  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 6px;
    border-radius: var(--radius-card);
    text-align: center;
    color: inherit;
  }
  .card:focus-visible {
    outline: none;
  }
  .card:focus-visible .record {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 4px var(--accent),
      0 16px 30px -18px rgba(0, 0, 0, 0.8);
  }
  /* The disc: fine grooves, a lead-in ring, a light sheen that turns on hover
     and the portrait as the label around the spindle. */
  .record {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 1;
    border-radius: 50%;
    background:
      radial-gradient(circle, transparent 0 66%, rgba(255, 255, 255, 0.05) 66.4% 66.8%, transparent 67.2%),
      repeating-radial-gradient(circle, var(--vinyl) 0 1px, var(--vinyl-groove) 1px 2px);
    box-shadow:
      0 16px 30px -18px rgba(0, 0, 0, 0.8),
      inset 0 0 0 1px rgba(255, 255, 255, 0.05);
    transition:
      transform var(--dur-state) var(--ease-enter),
      box-shadow var(--dur-state) var(--ease-enter);
  }
  .record::before {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: conic-gradient(
      from 20deg,
      transparent 0deg,
      var(--vinyl-sheen) 30deg,
      transparent 64deg,
      transparent 180deg,
      var(--vinyl-sheen) 210deg,
      transparent 244deg
    );
    pointer-events: none;
    transition: transform var(--dur-view) var(--ease-enter);
  }
  .label {
    position: absolute;
    inset: 17%;
    overflow: hidden;
    border-radius: 50%;
    background: linear-gradient(var(--surface-3), var(--surface-3)), var(--vinyl);
    box-shadow:
      0 0 0 3px var(--vinyl),
      0 0 0 4px rgba(255, 255, 255, 0.06);
  }
  .label :global(.cover) {
    width: 100%;
    height: 100%;
  }
  .label :global(img) {
    transition: transform var(--dur-view) var(--ease-enter);
  }
  /* Spindle hole. */
  .record::after {
    content: "";
    position: absolute;
    inset: calc(50% - 4px);
    border-radius: 50%;
    background: var(--vinyl);
    box-shadow:
      inset 0 1px 1px rgba(0, 0, 0, 0.8),
      0 0 0 1px rgba(255, 255, 255, 0.18);
  }
  .card:hover .record {
    transform: translateY(-3px);
    box-shadow:
      0 0 0 5px color-mix(in srgb, var(--accent) 14%, transparent),
      0 20px 34px -18px rgba(0, 0, 0, 0.85);
  }
  .card:hover .record::before {
    transform: rotate(48deg);
  }
  .card:hover .label :global(img) {
    transform: scale(1.05);
  }
  .title {
    max-width: 100%;
    font-size: 14.5px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
