<script lang="ts">
  import type { PageItem } from "../api";
  import { isDecadeRow, linkHues } from "../pageLinks";

  /// A row of page links (genres, moods, decades) as name tiles: TIDAL sends
  /// them without artwork, so each tile is a colour wash hued from its name.
  /// The grid wraps (no carousel); decade rows set their names as numerals.
  let {
    items,
    label,
    open,
  }: {
    items: PageItem[];
    label: string;
    open: (item: PageItem) => (() => void) | undefined;
  } = $props();

  const decades = $derived(isDecadeRow(items));
</script>

<ul class="links pane" class:decades aria-label={label}>
  {#each items as item, i (item.api_path ?? `${item.title}-${i}`)}
    {@const [h1, h2] = linkHues(item.title)}
    {@const onclick = open(item)}
    <li>
      {#if onclick}
        <button type="button" class="tile" style:--h1={h1} style:--h2={h2} {onclick}>
          <span class="name">{item.title}</span>
        </button>
      {:else}
        <span class="tile inert" style:--h1={h1} style:--h2={h2}>
          <span class="name">{item.title}</span>
        </span>
      {/if}
    </li>
  {/each}
</ul>

<style>
  .links {
    list-style: none;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(176px, 1fr));
    gap: 12px;
    margin: 0 0 24px;
    padding: 2px;
  }
  .links.decades {
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  }
  li {
    min-width: 0;
  }
  /* The wash: two OKLCH stops hued from the name over the theme's surface,
     inside the lit edge. Lightness/chroma/alpha are per-theme tokens. */
  .tile {
    --wash-a: oklch(var(--link-wash-l) var(--link-wash-c) var(--h1) / var(--link-wash-a));
    --wash-b: oklch(var(--link-wash-l) var(--link-wash-c) var(--h2) / calc(var(--link-wash-a) * 0.45));
    position: relative;
    display: flex;
    align-items: flex-end;
    width: 100%;
    aspect-ratio: 2.2 / 1;
    padding: 12px 14px;
    overflow: hidden;
    isolation: isolate;
    text-align: left;
    color: var(--text);
    border: 1px solid transparent;
    border-radius: var(--radius-card);
    background:
      linear-gradient(135deg, var(--wash-a), var(--wash-b)) padding-box,
      linear-gradient(var(--surface-2), var(--surface-2)) padding-box,
      var(--edge-lit) border-box;
    box-shadow: var(--shadow-panel);
    transition: transform var(--dur-state) var(--ease-enter);
  }
  /* A record's edge rising out of the corner: the music motif, in the
     second hue, so tiles read as a set without any artwork. */
  .tile::before {
    content: "";
    position: absolute;
    z-index: -1;
    right: -14%;
    bottom: -70%;
    height: 150%;
    aspect-ratio: 1;
    border-radius: 50%;
    border: 12px solid var(--wash-a);
    box-shadow: inset 0 0 0 10px var(--wash-b);
    opacity: 0.8;
    transition: transform var(--dur-view) var(--ease-enter);
  }
  button.tile:hover,
  button.tile:focus-visible {
    transform: translateY(-2px);
  }
  button.tile:hover::before,
  button.tile:focus-visible::before {
    transform: translate(-4%, -6%);
  }
  .tile.inert {
    opacity: 0.6;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 15px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .decades .tile {
    align-items: center;
  }
  .decades .name {
    font-family: var(--font-display);
    font-size: 30px;
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
  }
</style>
