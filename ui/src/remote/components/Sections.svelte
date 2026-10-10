<script lang="ts">
  import type { PageSection } from "../../lib/api";
  import { classifySection, isPlayableTrack } from "../../lib/media";
  import { useRemote } from "../context";
  import { fromPageItem, tileForItem, type Tile } from "../convert";
  import Shelf from "./Shelf.svelte";
  import TrackRow from "./TrackRow.svelte";

  let { sections }: { sections: PageSection[] } = $props();
  const { actions, store } = useRemote();

  const LIST_CAP = 8;
  const playingId = $derived(store.state?.current?.trackId ?? null);

  function playTile(tile: Tile) {
    if (tile.track) void actions.play([tile.track]);
  }
</script>

{#each sections as section, si (`${section.title}-${si}`)}
  {#if section.items.length > 0}
    {#if classifySection(section.kind, section.items) === "list"}
      {@const rows = section.items.filter(isPlayableTrack).slice(0, LIST_CAP).map(fromPageItem)}
      <h2 class="section-title">{section.title}</h2>
      {#each rows as row, i (`${row.id}-${i}`)}
        <TrackRow
          track={row}
          index={i}
          active={row.id === playingId}
          onplay={() => actions.play(rows, i)}
          onmenu={() => actions.openTrack(row)}
        />
      {/each}
    {:else}
      <Shelf title={section.title} tiles={section.items.map(tileForItem)} onplay={playTile} />
    {/if}
  {/if}
{/each}
