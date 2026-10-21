<script lang="ts">
  import type { ArtistRef } from "../api";
  import NameLink from "./NameLink.svelte";

  /// "A, B, C" with each artist its own link (name-links spec NL-D2). The
  /// artist whose page is showing (`current`) stays plain text. The separator
  /// is an expression: Svelte trims a block's trailing space, so a literal
  /// `, ` would render as ",".
  let {
    artists,
    onOpen,
    current = null,
    focusable = true,
    nested = false,
  }: {
    artists: ArtistRef[];
    onOpen?: (id: number) => void;
    current?: number | null;
    focusable?: boolean;
    nested?: boolean;
  } = $props();
</script>

{#each artists as artist, i (`${artist.id}-${i}`)}{#if i > 0}{", "}{/if}<NameLink
    label={artist.name}
    onOpen={onOpen && artist.id !== current ? () => onOpen(artist.id) : undefined}
    {focusable}
    {nested}
  />{/each}
