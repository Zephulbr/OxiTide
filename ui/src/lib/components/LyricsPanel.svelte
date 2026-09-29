<script lang="ts">
  import type { LyricsDto } from "../api";
  import { appearance } from "../appearance.svelte";
  import { activeLine } from "../lyrics";
  import type { Player } from "../player.svelte";
  import Icon from "./Icon.svelte";

  let { player, dto }: { player: Player; dto: LyricsDto } = $props();

  let scroller = $state<HTMLElement | null>(null);
  let following = $state(true);
  let lineEls: HTMLElement[] = [];

  const active = $derived(activeLine(dto.lines, player.state.position_ms));

  // Keep the active line ~40% from the top; users who scroll pause this.
  $effect(() => {
    const index = active;
    const container = scroller;
    if (!dto.synced || !following || index < 0 || !container) return;
    const el = lineEls[index];
    if (!el) return;
    container.scrollTo({
      top: Math.max(0, el.offsetTop - container.clientHeight * 0.4),
      behavior: appearance.reducedMotion ? "auto" : "smooth",
    });
  });

  function seek(line: { time_ms: number | null }) {
    if (line.time_ms === null) return;
    following = true;
    void player.seek(line.time_ms);
  }
</script>

<div class="lyrics-wrap">
  <div
    class="lyrics"
    class:rtl={dto.rtl}
    role="region"
    aria-label="Lyrics"
    bind:this={scroller}
    onwheel={() => (following = false)}
    onpointerdown={() => (following = false)}
    ontouchstart={() => (following = false)}
  >
    {#each dto.lines as line, i (i)}
      <button
        class="line"
        class:active={dto.synced && i === active}
        class:tappable={dto.synced && line.time_ms !== null}
        bind:this={lineEls[i]}
        disabled={!dto.synced || line.time_ms === null}
        onclick={() => seek(line)}
      >
        {line.text}
      </button>
    {/each}
  </div>
  {#if dto.synced && !following}
    <button class="jump" onclick={() => (following = true)} aria-label="Jump to current line">
      <Icon name="locate-fixed" size={16} />
    </button>
  {/if}
</div>

<style>
  .lyrics-wrap {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .lyrics {
    position: relative;
    flex: 1;
    overflow-y: auto;
    overscroll-behavior: contain;
    mask-image: linear-gradient(180deg, transparent 0, #000 10%, #000 90%, transparent 100%);
    scrollbar-width: none;
  }
  .lyrics::-webkit-scrollbar {
    width: 0;
    height: 0;
  }
  /* Room for the first and last lines to rest on the 40% mark. */
  .lyrics::before {
    content: "";
    display: block;
    height: 40%;
  }
  .lyrics::after {
    content: "";
    display: block;
    height: 60%;
  }
  .line {
    display: block;
    width: 100%;
    margin: 0;
    padding: 6px 0;
    border: none;
    background: none;
    color: var(--text-muted);
    font: inherit;
    font-size: 15px;
    line-height: 1.45;
    text-align: start;
    white-space: pre-wrap;
    transition: color var(--dur-view) var(--ease-in);
  }
  .line.tappable {
    cursor: pointer;
  }
  .line.active {
    color: var(--text);
    font-weight: 600;
  }
  .line:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 2px;
  }
  .jump {
    position: absolute;
    right: 0;
    bottom: 4px;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 1px solid var(--line);
    background: var(--surface-float);
    color: var(--text);
    cursor: pointer;
  }
</style>
