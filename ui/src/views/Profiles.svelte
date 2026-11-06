<script lang="ts">
  import { untrack } from "svelte";
  import type { Artist } from "../lib/api";
  import { load as loadCached, peek } from "../lib/viewCache";
  import { favoriteArtistsSource, myProfileSource } from "../lib/viewData";
  import { groupArtists, visibleArtists, type ArtistSort } from "../lib/artistSort";
  import { ownCard, type OwnCard, type OwnProfile } from "../lib/ownProfile";
  import { profilePicture } from "../lib/profilePicture.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import SortSelect from "../lib/components/SortSelect.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import { appearance } from "../lib/appearance.svelte";

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

  // The account's own profile leads the page: it is never a favorite (TIDAL
  // has no following yourself), so the favorites list alone never shows it.
  // Optional: the page shows the favorites while it loads or if it fails.
  const meSource = myProfileSource();
  let me = $state<OwnProfile | null>(peek(meSource) ?? null);
  // A picture just changed in the editor shows before TIDAL's record follows.
  const mine = $derived(
    ownCard(
      me?.artist
        ? { ...me, artist: { ...me.artist, picture: profilePicture.for(me.artist.id, me.artist.picture) } }
        : me,
    ),
  );
  const others = $derived.by(() => {
    const ownId = mine?.artistId ?? null;
    return ownId === null ? artists : artists.filter((artist) => artist.id !== ownId);
  });
  const mineShown = $derived(
    mine && mine.name.toLowerCase().includes(query.trim().toLowerCase()) ? mine : null,
  );

  const shown = $derived(visibleArtists(others, query, sortMode));
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

  async function loadMe() {
    try {
      await loadCached(meSource, (value) => (me = value));
    } catch {
      // No own card; the favorites still show.
    }
  }

  $effect(() => {
    untrack(() => {
      void load();
      void loadMe();
    });
  });
</script>

<!-- Each artist is a round portrait; in the Hi-Fi Rack it is a record, the
     portrait being the label on a vinyl disc. In name order the letters are
     crate dividers. -->
{#snippet face(picture: string | null | undefined)}
  <span class="record" aria-hidden="true">
    <span class="label"><Cover uuid={picture} size={320} radius={160} alt="" /></span>
  </span>
{/snippet}

{#snippet records(list: Artist[])}
  {#each list as artist (artist.id)}
    <button
      class="card"
      class:vinyl={appearance.vinylRecords}
      onclick={() => nav.goto({ name: "artist", id: artist.id })}
    >
      {@render face(artist.picture ?? artist.cover_fallback)}
      <span class="title">{artist.name}</span>
    </button>
  {/each}
{/snippet}

<!-- The own profile: the first record, captioned "Your profile". Without an
     artist TIDAL has no page for it, so the card is not a button. -->
{#snippet own(card: OwnCard)}
  {#if card.artistId != null}
    {@const artistId = card.artistId}
    <button
      class="card own"
      class:vinyl={appearance.vinylRecords}
      onclick={() => nav.goto({ name: "artist", id: artistId })}
    >
      {@render face(card.picture)}
      <span class="caption">
        <span class="title">{card.name}</span>
        <span class="you">Your profile</span>
      </span>
    </button>
  {:else}
    <div class="card own static" class:vinyl={appearance.vinylRecords}>
      {@render face(card.picture)}
      <span class="caption">
        <span class="title">{card.name}</span>
        <span class="you">Your profile</span>
      </span>
    </div>
  {/if}
{/snippet}

<div class="profiles">
  <header class="page-head">
    <!-- The count sits under the title, as on Albums and Playlists. -->
    <div class="title-block">
      <h2>Profiles</h2>
      <!-- Space held while loading (no "0 artists" flash, no header reflow). -->
      <span
        class="summary tnum"
        class:arrive={arrive && !loading}
        style:visibility={loading || error ? "hidden" : undefined}
      >{others.length} {others.length === 1 ? "artist" : "artists"}</span>
    </div>
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
  {:else if others.length === 0 && !mine}
    <EmptyState message="No artists yet." />
  {:else if shown.length === 0 && !mineShown}
    <p class="muted empty">No artists match “{query.trim()}”.</p>
  {:else if groups}
    {#if mineShown}
      <div class="grid pane own-row" class:arrive>{@render own(mineShown)}</div>
    {/if}
    {#each groups as group (group.letter)}
      <h3 class="divider" class:arrive>
        <span class="tab">{group.letter}</span>
        <span class="muted n tnum">{group.artists.length}</span>
      </h3>
      <div class="grid pane" class:arrive>{@render records(group.artists)}</div>
    {/each}
  {:else}
    <div class="grid pane" class:arrive>
      {#if mineShown}{@render own(mineShown)}{/if}
      {@render records(shown)}
    </div>
  {/if}
</div>

<style>
  /* Albums' inset and shelf: a wider gutter than the shared page one, so the
     grid sits off the sidebar and the window edge. */
  .profiles {
    container-type: inline-size;
    padding-inline: 24px;
    padding-bottom: 32px;
  }
  .title-block {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .title-block h2 {
    margin: 0;
  }
  .summary {
    color: color-mix(in srgb, var(--text) 62%, transparent);
    font-size: 14px;
  }
  /* The shared page head, scrolling with the page as on Albums and
     Playlists, so the search and sort fields always sit on the page
     background, never over the portraits. */
  .filter {
    flex: 0 1 240px;
    min-width: 140px;
  }
  .empty {
    margin-top: 48px;
  }
  /* Albums' columns: eight across a wide window, stepping down as the column
     narrows. A container query, not a JS count: the queue dock's glide
     measures layout synchronously (AGENTS.md). */
  .grid {
    display: grid;
    grid-template-columns: repeat(var(--cols, 8), minmax(0, 1fr));
    gap: 18px 14px;
  }
  .divider + .grid {
    margin-top: 16px;
  }
  @container (max-width: 1240px) {
    .grid { --cols: 7; }
  }
  @container (max-width: 1080px) {
    .grid { --cols: 6; }
  }
  @container (max-width: 920px) {
    .grid { --cols: 5; }
  }
  @container (max-width: 760px) {
    .grid { --cols: 4; }
  }
  @container (max-width: 600px) {
    .grid { --cols: 3; }
  }
  @container (max-width: 440px) {
    .grid { --cols: 2; }
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
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
    text-align: center;
  }
  .n {
    padding-bottom: 7px;
    font-size: 12px;
    font-weight: 500;
  }

  /* The album tile's frame: 10px padding, a 12px gap to the caption. */
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 10px;
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
  /* Default: the portrait fills a plain circle, seated by a hairline. */
  .record {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 1;
    border-radius: 50%;
    box-shadow: 0 0 0 1px var(--line), var(--shadow-cover);
  }
  .label {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: 50%;
    background: var(--surface-3);
  }
  .label :global(.cover) {
    width: 100%;
    height: 100%;
  }
  /* Hover: only the image eases in, inside its circle. */
  .label :global(img) {
    transition: transform var(--dur-view) var(--ease-enter);
  }
  .card:hover .label :global(img) {
    transform: scale(1.06);
  }

  /* Hi-Fi Rack: the disc. Fine grooves, a lead-in ring, a light sheen that
     turns on hover and the portrait as the label around the spindle. */
  .vinyl .record {
    background:
      radial-gradient(circle, transparent 0 66%, rgba(255, 255, 255, 0.05) 66.4% 66.8%, transparent 67.2%),
      repeating-radial-gradient(circle, var(--vinyl) 0 1px, var(--vinyl-groove) 1px 2px);
    box-shadow:
      0 16px 30px -18px rgba(0, 0, 0, 0.8),
      inset 0 0 0 1px rgba(255, 255, 255, 0.05);
  }
  .vinyl .record::before {
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
  .vinyl .label {
    inset: 17%;
    background: linear-gradient(var(--surface-3), var(--surface-3)), var(--vinyl);
    box-shadow:
      0 0 0 3px var(--vinyl),
      0 0 0 4px rgba(255, 255, 255, 0.06);
  }
  /* Spindle hole. */
  .vinyl .record::after {
    content: "";
    position: absolute;
    inset: calc(50% - 4px);
    border-radius: 50%;
    background: var(--vinyl);
    box-shadow:
      inset 0 1px 1px rgba(0, 0, 0, 0.8),
      0 0 0 1px rgba(255, 255, 255, 0.18);
  }
  .vinyl.card:hover .record::before {
    transform: rotate(48deg);
  }
  /* Albums' caption type. The name underlines on hover like every other
     name link (NameLink). */
  .title {
    max-width: 100%;
    font-size: 15px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-underline-offset: 2px;
  }
  .card:hover .title {
    text-decoration: underline;
  }
  /* The own card: the name over a quiet "Your profile" line. It has no page
     without an artist, so it does not react to hover then. */
  .caption {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    max-width: 100%;
    min-width: 0;
  }
  .you {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-muted);
  }
  .static {
    cursor: default;
  }
  .static:hover .title {
    text-decoration: none;
  }
  .static:hover .label :global(img) {
    transform: none;
  }
  .vinyl.static:hover .record::before {
    transform: none;
  }
  /* Name order: the own card on its own row above the crate dividers. */
  .own-row + .divider {
    margin-top: 28px;
  }
</style>
