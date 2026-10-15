<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    kicker,
    title,
    subtitle = "",
    stats = "",
    cover,
    actions,
    tools,
    onPlay,
    onShuffle,
    disabled = false,
  }: {
    kicker: string;
    title: string;
    subtitle?: string;
    stats?: string;
    cover: Snippet;
    actions?: Snippet;
    /// Optional extra control in the transport row after Shuffle (playlist's
    /// expanding track search).
    tools?: Snippet;
    onPlay: () => void;
    onShuffle: () => void;
    disabled?: boolean;
  } = $props();
</script>

<header class="detail-header pane">
  <div class="hero">
    <span class="hero-art">{@render cover()}</span>
    <div class="hero-meta">
      <span class="kicker">{kicker}</span>
      <h2><span class="title-track">{title}</span></h2>
      {#if subtitle}<p class="subtitle">{subtitle}</p>{/if}
      {#if stats}<p class="stats tnum">{stats}</p>{/if}
    </div>
  </div>
  <div class="controls">
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
    flex: 0 0 auto;
    width: var(--hero-size);
  }
  .hero-meta {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    max-width: 512px;
    padding-bottom: 2px;
  }
  .kicker {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  h2 {
    margin: 0;
    font-size: clamp(28px, 3.2vw, 42px);
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
  .stats {
    margin: 0;
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 16px;
    margin-top: 24px;
  }
  .tools {
    display: flex;
    align-items: center;
  }
  /* Resting width matches the cover so their right edges line up; the search
     pill can still grow the row when it expands (max-content, not fixed). */
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
      background var(--dur-micro) var(--ease-in),
      color var(--dur-micro) var(--ease-in),
      filter var(--dur-micro) var(--ease-in),
      transform var(--dur-micro) var(--ease-in);
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
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
</style>
