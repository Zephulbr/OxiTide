<script lang="ts">
  import type { BioSegment } from "../api";

  let {
    segments,
    onArtist,
  }: { segments: BioSegment[]; onArtist?: (id: number) => void } = $props();
</script>

{#each segments as segment, i (i)}
  {#if segment.artistId !== null && segment.artistId !== undefined && onArtist}
    <button class="bio-link" onclick={() => onArtist(segment.artistId!)}>{segment.text}</button>
  {:else}
    <span>{segment.text}</span>
  {/if}
{/each}

<style>
  .bio-link {
    font: inherit;
    color: var(--accent);
    background: none;
    border: 0;
    padding: 0;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
</style>
