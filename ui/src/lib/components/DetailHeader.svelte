<script lang="ts">
  import type { Snippet } from "svelte";
  import { dragSource } from "../dragDrop.svelte";
  import type { DragPayload } from "../dragPayload";
  import { marquee } from "../marquee";
  import Icon from "./Icon.svelte";

  let {
    kicker,
    title,
    subtitle = "",
    stats = "",
    byline,
    cover,
    actions,
    tools,
    onPlay,
    onShuffle,
    disabled = false,
    drag,
  }: {
    kicker: string;
    title: string;
    subtitle?: string;
    stats?: string;
    /// Rich subtitle (e.g. linked artists); replaces `subtitle` when given.
    byline?: Snippet;
    cover: Snippet;
    actions?: Snippet;
    /// Optional extra control in the transport row after Shuffle (playlist's
    /// expanding track search).
    tools?: Snippet;
    onPlay: () => void;
    onShuffle: () => void;
    disabled?: boolean;
    /// What dragging the cover carries (the page's collection), for a drop
    /// on a playlist or the queue.
    drag?: () => DragPayload | null;
  } = $props();

  /// The stats line split so its counts ("702 TRACKS", "2 FANS") can take the
  /// accent; odd entries are the counts.
  const statParts = $derived(stats.split(/(\d[\d,.]*\s+(?:TRACKS?|FANS?)\b)/));
</script>

<header class="detail-header pane">
  <div class="hero">
    <span class="hero-art" use:dragSource={() => drag?.() ?? null}>
      {@render cover()}
      <span class="kicker">{kicker}</span>
    </span>
    <div class="hero-meta">
      <div class="titles">
        {#if stats}
          <p class="stats tnum">
            {#each statParts as part, i (i)}{#if i % 2}<span class="count">{part}</span>{:else}{part}{/if}{/each}
          </p>
        {/if}
        {#if byline}<p class="subtitle">{@render byline()}</p>
        {:else if subtitle}<p class="subtitle">{subtitle}</p>{/if}
        <!-- A long title ends in an ellipsis; hovering slides it along
             (lib/marquee.ts, as the player bar's lines). -->
        <h2 use:marquee><span class="title-line" data-marquee><span>{title}</span></span></h2>
      </div>
      <div class="transport">
        <button class="btn-play" onclick={onPlay} {disabled}>
          <Icon name="play" size={20} /> Play
        </button>
        <button class="btn-shuffle" onclick={onShuffle} {disabled}>
          <Icon name="shuffle" size={20} /> Shuffle
        </button>
        {#if tools}
          <span class="tools">{@render tools()}</span>
        {/if}
      </div>
    </div>
    {#if actions}
      <div class="actions">{@render actions()}</div>
    {/if}
  </div>
</header>

<style>
  /* A size container: the main column narrows with the window, the sidebar
     and a docked queue, and the tiers at the end fit the hero to it. */
  .detail-header {
    --hero-size: 320px;
    container: detail / inline-size;
    margin-bottom: 40px;
  }
  /* Cover, then title + transport, then the page's actions. `--art` is the
     cover's drawn size: the hero size (Rack sets a smaller one), capped by
     the narrow tiers. */
  .hero {
    --art: var(--hero-size);
    display: grid;
    grid-template-columns: var(--art) minmax(0, 1fr) auto;
    grid-template-areas: "art meta actions";
    column-gap: 40px;
  }
  .hero-art {
    grid-area: art;
    position: relative;
    width: var(--art);
  }
  /* As tall as the cover: the transport sits on its bottom edge and the
     title block sits a fixed step above it. */
  .hero-meta {
    grid-area: meta;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .titles {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: auto;
    margin-bottom: 28px;
  }
  h2,
  .subtitle,
  .stats {
    max-width: 560px;
  }
  /* Runs down the cover's right edge in the gap beside it, centred on it,
     reading top to bottom. */
  .kicker {
    position: absolute;
    top: 50%;
    left: calc(100% + 8px);
    writing-mode: vertical-rl;
    transform: translateY(-50%);
    line-height: 1;
    white-space: nowrap;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  h2 {
    margin: 0;
    font-size: clamp(30px, 3.6vw, 48px);
    line-height: 1.1;
  }
  .title-line {
    display: block;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    padding-bottom: 0.1em;
  }
  .subtitle {
    margin: 0;
    font-size: 15px;
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
  .subtitle :global(.name-link:hover) {
    color: var(--text);
  }
  .stats {
    margin: 0;
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .stats .count {
    color: var(--accent);
  }
  .tools {
    display: flex;
    align-items: center;
  }
  /* Under the title, bottom-aligned with the cover, so the track list starts
     right below the cover. Resting width is the cover's, so Play/Shuffle keep
     their size; the search pill can still grow the row when it expands
     (max-content, not fixed). */
  .transport {
    display: flex;
    align-items: center;
    gap: 10px;
    width: max-content;
    min-width: var(--art);
  }
  .transport > button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 9px;
    flex: 1 1 calc((var(--art) - 68px) / 2);
    height: 48px;
    padding: 0 24px;
    border-radius: var(--radius-control);
    font-weight: 600;
    transition:
      background var(--dur-micro) var(--ease-enter),
      color var(--dur-micro) var(--ease-enter),
      filter var(--dur-micro) var(--ease-enter),
      transform var(--dur-micro) var(--ease-enter);
  }
  .btn-play {
    background: var(--accent);
    color: var(--on-accent);
  }
  .btn-play:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .btn-play :global(svg) {
    fill: currentColor;
  }
  .btn-shuffle {
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .btn-shuffle:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface-3);
  }
  /* The page's own actions stay at the row's right end, level with Play. */
  .actions {
    grid-area: actions;
    align-self: end;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  /* Narrower: a smaller cover first; then the actions drop under Play and
     Shuffle (the cover's bottom edge still lines up with the last row); a
     column too narrow to sit anything beside the cover stacks it all. */
  @container detail (width < 1000px) {
    .hero {
      --art: min(var(--hero-size), 240px);
      column-gap: 32px;
    }
  }
  @container detail (width < 840px) {
    .hero {
      --art: min(var(--hero-size), 200px);
      grid-template-columns: var(--art) minmax(0, 1fr);
      grid-template-rows: 1fr auto;
      grid-template-areas:
        "art meta"
        "art actions";
      row-gap: 14px;
    }
    .hero-art {
      align-self: end;
    }
    .actions {
      justify-self: start;
    }
    .titles {
      margin-bottom: 20px;
    }
  }
  @container detail (width < 520px) {
    .hero {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: none;
      grid-template-areas:
        "art"
        "meta"
        "actions";
      row-gap: 20px;
    }
    .titles {
      margin-top: 0;
    }
    .transport {
      width: auto;
      min-width: 0;
    }
  }
</style>
