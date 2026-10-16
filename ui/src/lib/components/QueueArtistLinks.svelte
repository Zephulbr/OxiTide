<script lang="ts">
  import { api } from "../api";
  import type { Route } from "../nav.svelte";
  import { primaryArtistRoute, type LinkSource } from "../queueLinks";
  import ArtistLinks from "./ArtistLinks.svelte";
  import NameLink from "./NameLink.svelte";

  /// A queue/history item's artists: one link each when the item carries ids,
  /// else the joined string as a single link to the primary artist, resolved
  /// on click (name-links spec NL-D8).
  let {
    item,
    go,
    focusable = true,
    nested = false,
  }: {
    item: LinkSource;
    go: (route: Route) => void;
    focusable?: boolean;
    nested?: boolean;
  } = $props();

  async function openPrimary() {
    const route = await primaryArtistRoute(item, api.getTrack);
    if (route) go(route);
  }
</script>

{#if item.artist_refs && item.artist_refs.length > 0}
  <ArtistLinks artists={item.artist_refs} onOpen={(id) => go({ name: "artist", id })} {focusable} {nested} />
{:else if item.artist}
  <NameLink label={item.artist} onOpen={() => void openPrimary()} {focusable} {nested} />
{/if}
