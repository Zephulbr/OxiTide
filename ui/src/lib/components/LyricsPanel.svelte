<script lang="ts">
  import { untrack } from "svelte";
  import type { LyricsDto } from "../api";
  import { appearance } from "../appearance.svelte";
  import { activeLine, lineWords, LYRICS_LOOKAHEAD_MS } from "../lyrics";
  import type { Player } from "../player.svelte";
  import Icon from "./Icon.svelte";

  let { player, dto }: { player: Player; dto: LyricsDto } = $props();

  let scroller = $state<HTMLElement | null>(null);
  let following = $state(true);
  let lineEls: HTMLElement[] = [];

  // Position events arrive every 500ms; this local clock advances between them
  // so the karaoke fill lands on time. It resets on every event and on a seek,
  // and freezes on pause. Only the synced view needs it. Seeded with the
  // current position so the first centering uses the real active line.
  let liveMs = $state(untrack(() => player.pendingSeek ?? player.state.position_ms));
  $effect(() => {
    const base = player.pendingSeek ?? player.state.position_ms;
    const playing = player.state.playing;
    liveMs = base;
    if (!playing || !dto.synced) return;
    const started = performance.now();
    let raf = requestAnimationFrame(function tick(now) {
      liveMs = base + (now - started);
      raf = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(raf);
  });

  const active = $derived(activeLine(dto.lines, liveMs));

  // Where the song was when the panel mounted: entrance staggering and fill
  // tiers anchor to this so opening mid-song settles around the current line
  // instead of revealing the whole list at once.
  const initialActive = untrack(() =>
    activeLine(dto.lines, player.pendingSeek ?? player.state.position_ms),
  );
  const lineDelay = (i: number) =>
    appearance.reducedMotion || !dto.synced ? 0 : Math.min(Math.abs(i - initialActive) * 24, 140);
  const lineTarget = (i: number) => {
    if (!dto.synced) return 0.85;
    if (initialActive < 0) return 0.5;
    if (i === initialActive) return 1;
    return i < initialActive ? 0.3 : 0.5;
  };

  // Word timing for the karaoke fill, active line only; estimation needs the
  // next timed line as the end boundary.
  const activeWords = $derived.by(() => {
    if (!dto.synced || active < 0) return [];
    const line = dto.lines[active];
    if (line.time_ms === null) return [];
    // Out-of-order stamps are tolerated (activeLine picks by time), so the
    // boundary is the chronologically nearest later stamp, not the next entry.
    let nextStart: number | null = null;
    for (const candidate of dto.lines) {
      const time = candidate.time_ms;
      if (time !== null && time > line.time_ms && (nextStart === null || time < nextStart)) {
        nextStart = time;
      }
    }
    return lineWords(line, nextStart);
  });

  let positioned = false;

  // Keep the active line vertically centered; users who scroll pause this.
  $effect(() => {
    const index = active;
    const container = scroller;
    if (!dto.synced || !following || index < 0 || !container) return;
    const el = lineEls[index];
    if (!el) return;
    // The first positioning for a track jumps straight to the active line so
    // the entrance never races a scroll; later follows (and seeks) glide.
    const behavior: ScrollBehavior = positioned && !appearance.reducedMotion ? "smooth" : "auto";
    container.scrollTo({
      top: Math.max(0, el.offsetTop + el.offsetHeight / 2 - container.clientHeight / 2),
      behavior,
    });
    positioned = true;
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
    dir={dto.rtl ? "rtl" : "ltr"}
    role="region"
    aria-label="Lyrics"
    bind:this={scroller}
    onwheel={() => (following = false)}
    onpointerdown={() => (following = false)}
    ontouchstart={() => (following = false)}
  >
    {#each dto.lines as line, i (i)}
      {@const state = !dto.synced ? "plain" : active < 0 ? "next" : i === active ? "active" : i < active ? "past" : "next"}
      {@const far = dto.synced && active >= 0 && Math.abs(i - active) >= 4}
      <button
        class="line"
        class:active={state === "active"}
        class:past={state === "past"}
        class:next={state === "next"}
        class:plain={state === "plain"}
        class:far
        class:tappable={dto.synced && line.time_ms !== null}
        style="--line-delay: {lineDelay(i)}ms; --line-target: {lineTarget(i)}"
        bind:this={lineEls[i]}
        disabled={!dto.synced || line.time_ms === null}
        onclick={() => seek(line)}
      >
        {#if dto.synced && i === active && activeWords.length > 0}
          {#each activeWords as word, wi (wi)}
            <span class:sung={liveMs + LYRICS_LOOKAHEAD_MS >= word.time_ms}>{word.text}</span>
          {/each}
        {:else}
          {line.text}
        {/if}
      </button>
    {/each}
  </div>
  {#if dto.synced && !following}
    <button class="jump" onclick={() => (following = true)} aria-label="Jump to current line">
      <Icon name="circle-dot" size={16} />
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
    /* Pull the lyrics block toward the artwork. Transform-only so nothing
       reflows and no other element moves. */
    transform: translateX(-48px);
  }
  .lyrics {
    position: relative;
    flex: 1;
    overflow-y: auto;
    overscroll-behavior: contain;
    /* The active line is placed by the centering effect; keep the browser's
       scroll anchoring out of it when content reflows. */
    overflow-anchor: none;
    mask-image: linear-gradient(180deg, transparent 0, #000 10%, #000 90%, transparent 100%);
    scrollbar-width: none;
  }
  .lyrics::-webkit-scrollbar {
    width: 0;
    height: 0;
  }
  /* Room for the first and last lines to rest on the center mark. */
  .lyrics::before {
    content: "";
    display: block;
    height: 50%;
  }
  .lyrics::after {
    content: "";
    display: block;
    height: 50%;
  }
  /* Karaoke treatment: one bright text color, and opacity tiers carry the
     state — active full, upcoming half, already-sung dimmed further (a sense
     of progress), far-away lines softened with a depth blur. Unsynced lyrics
     skip the tiers entirely: a plain, readable list. */
  .line {
    display: block;
    width: 100%;
    margin: 0;
    padding: 12px 0;
    border: none;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: calc((var(--font-body-size) + 16px) * 1.1);
    font-weight: 700;
    line-height: 1.35;
    text-align: start;
    white-space: pre-wrap;
    opacity: 0.5;
    transition:
      opacity var(--dur-view) var(--ease-in-out),
      filter var(--dur-view) var(--ease-in-out),
      color var(--dur-view) var(--ease-in-out);
    /* Entrance: each line fades in on mount, delayed by its distance from the
       line the song is on (inline --line-delay) so opening mid-song settles
       around the current line instead of revealing the whole list at once.
       The inline --line-target matches the tier the classes set, so the fade
       lands on it and `backwards` holds the line invisible through its delay. */
    animation: line-in var(--dur-state) var(--ease-in-out) var(--line-delay, 0ms) backwards;
  }
  @keyframes line-in {
    from { opacity: 0; }
    to { opacity: var(--line-target, 1); }
  }
  @media (prefers-reduced-motion: reduce) {
    .line { animation: none; }
  }
  /* Two classes also beat app.css's `button:disabled` fade — an untimed line
     in a synced track is disabled but must still hold its tier. */
  .line.next {
    opacity: 0.5;
  }
  .line.past {
    opacity: 0.3;
  }
  .line.active {
    opacity: 1;
  }
  /* Two classes beat app.css's `button:disabled` fade for the plain list. */
  .line.plain {
    opacity: 0.85;
  }
  .line.far {
    filter: blur(2px);
  }
  /* Karaoke fill: each word flips to the accent as it is sung. */
  .line span {
    transition: color var(--dur-micro) var(--ease-enter);
  }
  .line span.sung {
    color: var(--accent);
  }
  .line.tappable {
    cursor: pointer;
  }
  /* Click-to-seek affordance: a tappable line brightens under the pointer. */
  .line.tappable:hover {
    opacity: 0.85;
  }
  .line.active.tappable:hover {
    opacity: 1;
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
