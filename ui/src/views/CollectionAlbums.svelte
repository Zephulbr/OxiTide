<script lang="ts">
  import { untrack } from "svelte";
  import type { Album, PageItem } from "../lib/api";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
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

<h2>Albums</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if albums.length === 0}
  <EmptyState message="No albums yet." />
{:else}
  <div class="grid pane" class:arrive>
    {#each albums as album (album.id)}
      {@const item: PageItem = {
        id: String(album.id),
        title: album.title,
        subtitle: album.artists.map((a) => a.name).join(", "),
        item_type: "ALBUM",
        artwork: album.cover ?? null,
        release_type: album.type ?? null,
        artist_refs: album.artists,
      }}
      <MediaCard {item} variant="tile" onclick={() => nav.goto({ name: "album", id: album.id })} />
    {/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
  }
</style>
