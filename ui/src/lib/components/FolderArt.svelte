<script lang="ts">
  import { untrack } from "svelte";
  import type { CollectionEntry } from "../api";
  import { collageCovers } from "../artwork";
  import { folderMeta } from "../folderMeta.svelte";
  import { playlistCovers, requestPlaylistCovers } from "../playlistArt.svelte";
  import { load as loadCached } from "../viewCache";
  import { collectionSource } from "../viewData";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  /// A folder's artwork on the Playlists grid: a seamless mosaic of the first
  /// covers of the playlists inside it, with a folder badge in the user's
  /// folder colour. An empty folder (or one still loading) is the coloured
  /// folder icon on a wash of the same hue.
  let { folderId }: { folderId: string } = $props();

  const colors = folderMeta();
  const hue = $derived(colors.hue(folderId));

  let children = $state<CollectionEntry[]>([]);
  $effect(() => {
    const id = folderId;
    untrack(() => {
      void loadCached(collectionSource(id, undefined), (list) => (children = list)).catch(() => {});
    });
  });

  /// One cover per playlist: the first collage tile of an owned playlist,
  /// else TIDAL's square art.
  const covers = $derived.by(() => {
    const picks: (string | null | undefined)[] = [];
    for (const entry of children) {
      if (entry.kind !== "playlist") continue;
      picks.push(entry.owned ? (playlistCovers(entry.uuid)?.[0] ?? entry.squareImage) : entry.squareImage);
      if (picks.length === 8) break;
    }
    return collageCovers(picks);
  });

  $effect(() => {
    for (const entry of children.slice(0, 8)) {
      if (entry.kind === "playlist" && entry.owned) untrack(() => requestPlaylistCovers(entry.uuid));
    }
  });
</script>

<span class="folder-art" class:tinted={hue !== null} style:--hue={hue ?? undefined}>
  {#if covers.length > 1}
    <span class="mosaic">
      {#each covers as cover, i (i)}
        <Cover uuid={cover} size={160} radius={0} alt="" />
      {/each}
    </span>
  {:else if covers.length === 1}
    <span class="mosaic single"><Cover uuid={covers[0]} size={320} radius={0} alt="" /></span>
  {:else}
    <span class="empty"><Icon name="folder" size={48} /></span>
  {/if}
  {#if covers.length > 0}
    <span class="badge" aria-hidden="true"><Icon name="folder" size={16} /></span>
  {/if}
</span>

<style>
  .folder-art {
    /* The folder's colour; without a pick it is the muted text colour. */
    --folder: var(--text-muted);
    position: relative;
    display: block;
    width: 100%;
    height: 100%;
  }
  .folder-art.tinted {
    --folder: oklch(var(--folder-l) var(--folder-c) var(--hue));
  }
  .mosaic {
    display: grid;
    grid-template-columns: 1fr 1fr;
    width: 100%;
    height: 100%;
  }
  .mosaic.single {
    display: block;
  }
  .mosaic :global(.cover) {
    width: 100%;
    height: 100%;
  }
  .empty {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    color: var(--folder);
    background: linear-gradient(
      135deg,
      color-mix(in srgb, var(--folder) 22%, var(--surface-2)),
      color-mix(in srgb, var(--folder) 8%, var(--surface-3))
    );
  }
  /* The folder mark: a glass chip in the corner, the icon in the folder's
     colour, so a folder never reads as just another playlist. */
  .badge {
    position: absolute;
    left: 8px;
    bottom: 8px;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-control);
    color: var(--folder);
    background: var(--surface-float);
    box-shadow: 0 0 0 1px var(--line), var(--shadow-cover);
    backdrop-filter: blur(var(--glass-blur));
  }
  /* WebKitGTK re-filters every backdrop on each composite, so a page of
     folders would cost what Home's mix badges did (MediaCard). The fill is
     mostly opaque; on Linux the chip drops the blur. */
  :global(:root[data-platform="linux"]) .badge {
    backdrop-filter: none;
  }
</style>
