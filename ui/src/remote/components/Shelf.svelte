<script lang="ts">
  import type { Tile } from "../convert";
  import { hrefFor } from "../routes";
  import Art from "./Art.svelte";

  let {
    title = "",
    tiles,
    grid = false,
    onplay,
  }: {
    title?: string;
    tiles: Tile[];
    /// Wrap into a grid instead of a horizontal scroller.
    grid?: boolean;
    /// Tapping a tile that is a track (no page of its own).
    onplay?: (tile: Tile) => void;
  } = $props();
</script>

{#if title}<h2 class="section-title">{title}</h2>{/if}
<ul class:grid class:scroller={!grid}>
  {#each tiles as tile, i (tile.key)}
    <li style:--delay={`${i < 10 ? i * 40 : 0}ms`}>
      {#if tile.route}
        <a href={hrefFor(tile.route)} class="tile">
          <Art src={tile.image} size="100%" round={tile.round} />
          <span class="title ellipsis" class:center={tile.round}>{tile.title}</span>
          {#if tile.subtitle}<span class="sub muted ellipsis" class:center={tile.round}>{tile.subtitle}</span>{/if}
        </a>
      {:else}
        <button class="tile" disabled={!tile.track || !onplay} onclick={() => onplay?.(tile)}>
          <Art src={tile.image} size="100%" round={tile.round} />
          <span class="title ellipsis">{tile.title}</span>
          {#if tile.subtitle}<span class="sub muted ellipsis">{tile.subtitle}</span>{/if}
        </button>
      {/if}
    </li>
  {/each}
</ul>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .scroller {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 42%;
    gap: 12px;
    overflow-x: auto;
    scroll-snap-type: x proximity;
    scroll-padding-inline: var(--remote-gutter);
    margin: 0 calc(-1 * var(--remote-gutter));
    padding: 0 var(--remote-gutter) 4px;
    scrollbar-width: none;
  }
  .scroller::-webkit-scrollbar {
    display: none;
  }
  li {
    animation: tile-in var(--dur-view) var(--ease-in) both;
    animation-delay: var(--delay, 0ms);
  }
  @keyframes tile-in {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.98);
    }
  }
  .tile:active :global(.art) {
    transform: scale(0.96);
  }
  .tile :global(.art) {
    transition: transform var(--dur-micro) var(--ease-in);
  }
  .scroller li {
    scroll-snap-align: start;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 16px 12px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
  }
  .tile :global(.art) {
    aspect-ratio: 1;
    height: auto !important;
    margin-bottom: 6px;
    border-radius: var(--radius-card);
  }
  .tile :global(.art.round) {
    border-radius: 50%;
  }
  .title {
    font-weight: 600;
    font-size: 14px;
  }
  .sub {
    font-size: 12px;
  }
  .center {
    text-align: center;
  }
</style>
