<script lang="ts">
  import type { Track } from "../api";
  import { collageCovers, type CoverSize } from "../artwork";
  import { playlistCovers, requestPlaylistCovers } from "../playlistArt.svelte";
  import Cover from "./Cover.svelte";

  let {
    owned,
    playlist,
    image,
    tracks,
    size = 320,
    radius = 12,
    alt = "",
  }: {
    /// Only user-created playlists get the collage; curated ones keep TIDAL art.
    owned: boolean;
    /// Playlist uuid; the collage store is keyed by it.
    playlist: string;
    /// TIDAL's square artwork, used while covers load and for curated playlists.
    image?: string | null;
    /// Track source for callers that already hold it (the playlist view);
    /// list rows omit it and read the session store instead.
    tracks?: Track[];
    size?: CoverSize;
    radius?: number;
    alt?: string;
  } = $props();

  const covers = $derived(
    !owned
      ? []
      : tracks
        ? collageCovers(tracks.map((t) => t.album?.cover))
        : (playlistCovers(playlist) ?? []),
  );

  $effect(() => {
    if (owned && !tracks && playlist && playlistCovers(playlist) === null) {
      requestPlaylistCovers(playlist);
    }
  });

  const tile = $derived(size >= 320 ? (160 as const) : (80 as const));
  const single = $derived(size >= 320 ? (320 as const) : (160 as const));
  const gap = $derived(size >= 320 ? 2 : 1);
</script>

{#if covers.length > 1}
  <div
    class="collage"
    role="img"
    aria-label={alt || undefined}
    style:border-radius="{radius}px"
    style:gap="{gap}px"
  >
    {#each covers as cover, i (i)}
      <Cover uuid={cover} size={tile} radius={0} alt="" />
    {/each}
  </div>
{:else}
  <Cover uuid={covers[0] ?? image ?? null} size={single} {radius} {alt} />
{/if}

<style>
  .collage {
    display: grid;
    grid-template-columns: 1fr 1fr;
    aspect-ratio: 1;
    overflow: hidden;
    background: var(--surface-2);
  }
</style>
