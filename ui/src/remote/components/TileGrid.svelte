<script lang="ts">
  import type { Tile } from "../convert";
  import { FILTER_MIN, filterTiles } from "../filter";
  import FilterBox from "./FilterBox.svelte";
  import Shelf from "./Shelf.svelte";
  import { t } from "../i18n.svelte";

  /// A Library grid (albums, artists, mixes) with a filter box once it has
  /// `FILTER_MIN` tiles. `subtitle` says whether the subtitle is content to
  /// match (an album's artists) or a kind label (an artist tile's "Artist").
  let { tiles, label, subtitle = true }: { tiles: Tile[]; label: string; subtitle?: boolean } = $props();

  let query = $state("");
  const shown = $derived(filterTiles(tiles, query, { subtitle }));
</script>

{#if tiles.length >= FILTER_MIN || query}<FilterBox bind:value={query} {label} />{/if}
{#if query.trim() && shown.length === 0}
  <p class="muted">{t("filter.noItems", { filter: query.trim() })}</p>
{/if}
<Shelf tiles={shown} grid />
