<script lang="ts">
  import { untrack } from "svelte";
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import type { Album, PageItem } from "../lib/api";
  import { sortAlbums, type AlbumSort } from "../lib/albumSort";
  import { appearance } from "../lib/appearance.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import SortSelect from "../lib/components/SortSelect.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import { load as loadCached, peek } from "../lib/viewCache";
  import { favoriteAlbumsSource } from "../lib/viewData";

  let { nav }: { nav: Nav } = $props();

  // Cached visits render complete on the first frame; a grid that had to
  // load rises in (`.arrive`) instead of snapping in.
  const source = favoriteAlbumsSource();
  const cached = peek(source);
  const arrive = cached === undefined;
  let albums = $state<Album[]>(cached ?? []);
  let loading = $state(cached === undefined);
  let error = $state("");

  let sortMode = $state<AlbumSort>("added");
  const rows = $derived(sortAlbums(albums, sortMode));

  async function load() {
    if (peek(source) === undefined) loading = true;
    error = "";
    try {
      await loadCached(source, (list) => (albums = list));
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    untrack(load);
  });
</script>

<div class="albums">
<header class="page-head">
  <!-- The count sits under the title rather than beside the tools, as on Playlists. -->
  <div class="title-block">
    <h2>Albums</h2>
    {#if !loading && !error && albums.length > 0}
      <span class="summary tnum">{albums.length} {albums.length === 1 ? "album" : "albums"}</span>
    {/if}
  </div>
  <SortSelect
    value={sortMode}
    options={[
      { value: "added", label: "Recently added" },
      { value: "released", label: "Release date" },
      { value: "title", label: "Title A–Z" },
      { value: "artist", label: "Artist A–Z" },
    ]}
    label="Sort albums"
    onchange={(value) => (sortMode = value as AlbumSort)}
  />
</header>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if albums.length === 0}
  <EmptyState message="No albums yet." />
{:else}
  <div class="grid pane" class:arrive role="list">
    {#each rows as album (album.id)}
      {@const item: PageItem = {
        id: String(album.id),
        title: album.title,
        subtitle: album.artists.map((a) => a.name).join(", "),
        item_type: "ALBUM",
        artwork: album.cover ?? null,
        release_type: album.type ?? null,
        artist_refs: album.artists,
      }}
      <!-- A sort change glides the cards to their new places, as on Playlists. -->
      <div
        class="cell"
        role="listitem"
        animate:flip={{ duration: appearance.reducedMotion ? 0 : 320, easing: cubicOut }}
      >
        <MediaCard {item} variant="tile" onclick={() => nav.goto({ name: "album", id: album.id })} />
      </div>
    {/each}
  </div>
{/if}
</div>

<style>
  /* A wider inset than the shared page gutter at the sides, so the shelf
     sits off the sidebar and the window edge. */
  .albums {
    container-type: inline-size;
    padding-inline: 24px;
  }
  /* Eight across a wide window, stepping down as the column narrows so a
     tile never drops much below ~140px. A container query, not a JS count:
     the queue dock's glide measures layout synchronously (AGENTS.md). */
  .grid {
    display: grid;
    grid-template-columns: repeat(var(--cols, 8), minmax(0, 1fr));
    gap: 18px 14px;
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
  .cell {
    min-width: 0;
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
  /* Playlists' card padding and caption type: at this cover size the
     shared tile's caption reads too small. */
  .grid :global(.media.tile) {
    padding: 10px;
  }
  .grid :global(.media .title) {
    font-size: 15px;
    font-weight: 600;
  }
  .grid :global(.media .sub) {
    font-size: 13px;
  }
</style>
