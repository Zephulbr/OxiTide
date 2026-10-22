<script lang="ts">
  import type { Snippet } from "svelte";
  import { dragSource } from "../dragDrop.svelte";
  import type { DragPayload } from "../dragPayload";
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
        <h2><span class="title-track">{title}</span></h2>
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
  .detail-header {
    --hero-size: 320px;
    margin-bottom: 40px;
  }
  .hero {
    display: flex;
    align-items: flex-end;
    gap: 40px;
  }
  .hero-art {
    position: relative;
    flex: 0 0 auto;
    width: var(--hero-size);
  }
  /* As tall as the cover: the transport sits on its bottom edge and the
     title block sits a fixed step above it. */
  .hero-meta {
    flex: 1;
    align-self: stretch;
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
    container-type: inline-size;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    padding-bottom: 0.1em;
  }
  .title-track {
    display: inline-block;
    width: max-content;
    transition: transform 2.5s var(--ease-in-out);
  }
  h2:hover {
    text-overflow: clip;
  }
  h2:hover .title-track {
    transform: translateX(min(0px, calc(100cqw - 100%)));
    transition-duration: 8s;
    transition-timing-function: linear;
  }
  @media (prefers-reduced-motion: reduce) {
    .title-track,
    h2:hover .title-track {
      transition: none;
    }
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
    min-width: var(--hero-size);
  }
  .transport > button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 9px;
    flex: 1 1 calc((var(--hero-size) - 68px) / 2);
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
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 4px;
  }
</style>
