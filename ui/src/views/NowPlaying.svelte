<script lang="ts">
  import { cubicOut } from "svelte/easing";
  import { fly, slide, type TransitionConfig } from "svelte/transition";
  import type { QueueItem } from "../lib/api";
  import { appearance } from "../lib/appearance.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import DiscArt from "../lib/components/DiscArt.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import LyricsPanel from "../lib/components/LyricsPanel.svelte";
  import { formatDuration } from "../lib/format";
  import { lyrics } from "../lib/lyrics.svelte";
  import { formatQuality, type QualityDisplay } from "../lib/quality";
  import type { Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";

  /// Outro pins the closing view absolutely over the incoming route, so the
  /// swap dissolves in place instead of reflowing .main mid-fade.
  function npOut(node: HTMLElement): TransitionConfig {
    node.style.position = "absolute";
    // matches .main padding in app.css (bottom clears the floating player bar)
    node.style.inset = "16px 20px calc(var(--player-height) + 8px) 20px";
    node.style.zIndex = "1";
    node.style.pointerEvents = "none";
    return { duration: 150, css: (t) => `opacity: ${t}` };
  }

  let {
    player,
    nav,
    qualityDisplay,
  }: { player: Player; nav: Nav; qualityDisplay: QualityDisplay } = $props();

  const track = $derived(player.state.current);
  const quality = $derived(
    player.state.stream ? formatQuality(player.state.stream, "compact", player.state.preferred_quality) : "",
  );
  const qualityDetail = $derived(
    player.state.stream ? formatQuality(player.state.stream, "detailed", player.state.preferred_quality) : "",
  );
  const nextUp = $derived.by(() => {
    const current = track;
    if (!current) return [];
    const index = player.state.queue.findIndex((item) => item.track_id === current.track_id);
    if (index < 0) return [];
    return player.state.queue.slice(index + 1, index + 4);
  });

  const showLyrics = $derived(lyrics.tab === "lyrics" && lyrics.lyrics !== null);
  const hasLyrics = $derived(lyrics.lyrics !== null);

  /// Transition duration that collapses to 0 when reduced motion is requested.
  const dur = (ms: number) => (appearance.reducedMotion ? 0 : ms);

  $effect(() => {
    void lyrics.open(track);
  });
</script>

{#snippet details(t: QueueItem)}
  <span class="eyebrow">Now playing</span>
  <h2 class="title">{t.title}</h2>
  <p class="byline">{t.artist}{t.album ? ` — ${t.album}` : ""}</p>
  <div class="badges">
    {#if quality}<span class="badge" class:max={quality === "MAX"}>{quality}</span>{/if}
    {#if player.state.bit_perfect}<span class="badge">Bit-perfect</span>{/if}
    {#if qualityDetail}<span class="badge muted">{qualityDetail}</span>{/if}
  </div>
{/snippet}

<div
  class="now-playing"
  class:with-lyrics={showLyrics}
  class:has-lyrics={hasLyrics}
  out:npOut
>
  <div class="stage">
    {#if track}
      <div class="stage-row">
        <div class="art">
          {#if appearance.coverShape === "disc"}
            <button class="art-disc" onclick={() => nav.closeNowPlaying()} aria-label="Close now playing">
              <DiscArt uuid={track.cover} playing={player.state.playing} />
            </button>
          {:else}
            <button class="art-sleeve lit" onclick={() => nav.closeNowPlaying()} aria-label="Close now playing">
              <Cover uuid={track.cover} size={640} radius={12} alt={`${track.title} artwork`} />
            </button>
          {/if}
        </div>
        {#if hasLyrics}
          <div class="spine" role="tablist" aria-orientation="vertical" aria-label="Now playing content">
            <button
              role="tab"
              aria-selected={lyrics.tab === "lyrics"}
              class:active={lyrics.tab === "lyrics"}
              onclick={() => lyrics.setTab("lyrics")}
            >
              Lyrics
            </button>
            <button
              role="tab"
              aria-selected={lyrics.tab === "next"}
              class:active={lyrics.tab === "next"}
              onclick={() => lyrics.setTab("next")}
            >
              Next up
            </button>
          </div>
        {/if}
      </div>
      {#if showLyrics}
        <div
          class="stage-details"
          in:slide={{ duration: dur(320), delay: dur(60), easing: cubicOut }}
          out:slide={{ duration: dur(220), easing: cubicOut }}
        >
          <div
            class="stage-details-in"
            in:fly={{ y: 16, duration: dur(300), delay: dur(60), easing: cubicOut }}
            out:fly={{ y: 16, duration: dur(170), easing: cubicOut }}
          >
            {@render details(track)}
          </div>
        </div>
      {/if}
    {:else}
      <div class="art-empty lit"><Icon name="music" size={64} /></div>
    {/if}
  </div>

  <div class="meta">
    {#if track}
      {#if showLyrics}
        {@const shown = lyrics.lyrics}
        {#if shown}
          <div
            class="lyrics-slot"
            in:fly={{ y: 16, duration: dur(260), delay: dur(180), easing: cubicOut }}
            out:fly={{ y: 24, duration: dur(200), easing: cubicOut }}
          >
            {#key shown.track_id}
              <LyricsPanel {player} dto={shown} />
            {/key}
          </div>
        {/if}
      {:else}
        <div
          class="meta-details"
          in:fly={{ y: -16, duration: dur(260), delay: dur(40), easing: cubicOut }}
          out:fly={{ y: -16, duration: dur(200), easing: cubicOut }}
        >
          {@render details(track)}
        </div>

        <hr
          in:fly={{ y: -16, duration: dur(240), delay: dur(60), easing: cubicOut }}
          out:fly={{ y: -16, duration: dur(160), easing: cubicOut }}
        />

        <div
          class="meta-list"
          in:fly={{ y: -16, duration: dur(240), delay: dur(80), easing: cubicOut }}
          out:fly={{ y: -16, duration: dur(160), easing: cubicOut }}
        >
          {#if lyrics.loading}
            <p class="muted loading">Loading lyrics…</p>
          {:else if nextUp.length > 0}
            <ul class="next-up">
              {#each nextUp as item (item.track_id)}
                <li>
                  <span class="thumb"><Cover uuid={item.cover} size={80} radius={6} alt="" /></span>
                  <span class="next-title">{item.title}</span>
                  <span class="muted next-artist">{item.artist}</span>
                  <span class="muted tnum">{formatDuration(item.duration_secs)}</span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted">Queue is empty</p>
          {/if}
        </div>
      {/if}
    {:else}
      <span class="eyebrow">Now playing</span>
      <p class="muted">Nothing playing</p>
    {/if}
  </div>
</div>

<style>
  .now-playing {
    --np-glide: 460ms;
    /* Shared with the lyrics mode so the cover never changes size. */
    --np-cover: min(60vh, calc(100vh - 360px));
    display: flex;
    gap: 32px;
    height: 100%;
    max-width: 1520px;
    margin: 0 auto;
    align-items: center;
    /* Known lengths animate so every piece of the morph glides together:
       the gap, the composition width, and the flex columns below. */
    transition:
      gap var(--np-glide) var(--ease-in-out),
      max-width var(--np-glide) var(--ease-in-out);
    /* Entrance: the view rises, the artwork scales up, then the meta column
       staggers in. Mount-only; closing is the npOut fade. */
    animation: np-in var(--dur-view) var(--ease-in) both;
  }
  @keyframes np-in {
    from { opacity: 0; transform: translateY(16px); }
  }
  .art {
    display: grid;
    place-items: center;
    flex: 0 1 auto;
    width: min(100%, var(--np-cover));
    min-width: 0;
    animation: np-art var(--dur-view) var(--ease-in) 60ms both;
  }
  @keyframes np-art {
    from { opacity: 0; transform: scale(0.92); }
  }
  @keyframes np-fade {
    from { opacity: 0; }
  }
  /* Reduced motion (spec §12): slides become fades, no stagger, no glide. */
  @media (prefers-reduced-motion: reduce) {
    .now-playing {
      --np-glide: 0ms;
    }
    .now-playing,
    .art {
      animation-name: np-fade;
      animation-delay: 0ms;
    }
  }
  /* Both shapes are one close affordance (spec §7.2): clicking the artwork
     returns, disc or sleeve. The disc button stays transparent (no lit edge). */
  .art-sleeve,
  .art-disc {
    display: block;
    width: 100%;
    margin: 0 auto;
  }
  .art-empty {
    width: min(60%, 320px);
    aspect-ratio: 1;
    display: grid;
    place-items: center;
    border-radius: var(--radius-card);
    color: var(--text-muted);
  }
  .eyebrow {
    display: block;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-muted);
    margin-bottom: 8px;
  }
  .title {
    font-family: var(--font-display);
    font-size: 31px;
    font-weight: 640;
    letter-spacing: -0.02em;
    margin: 0 0 6px;
  }
  .byline {
    font-family: var(--font-display);
    font-style: italic;
    font-size: 15.5px;
    font-weight: 520;
    color: var(--text-muted);
    margin: 0 0 16px;
  }
  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 16px;
  }
  .badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 8px;
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  .badge.max {
    border-color: var(--dyn-tertiary);
    color: var(--dyn-tertiary);
  }
  hr {
    border: none;
    border-top: 1px solid var(--line);
    margin: 16px 0;
  }
  .next-up {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .next-up li {
    display: grid;
    grid-template-columns: var(--thumb-size) minmax(0, 1fr) auto;
    grid-template-rows: auto auto;
    column-gap: 10px;
    align-items: center;
  }
  .thumb {
    grid-column: 1;
    grid-row: 1 / span 2;
    width: var(--thumb-size);
  }
  .next-title {
    grid-column: 2;
    grid-row: 1;
    font-size: 15.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .next-artist {
    grid-column: 2;
    grid-row: 2;
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .next-up .tnum {
    grid-column: 3;
    grid-row: 1 / span 2;
  }
  .stage {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 20px;
    flex: 1 1 0%;
    min-width: 0;
    min-height: 0;
    transition:
      flex-grow var(--np-glide) var(--ease-in-out),
      flex-shrink var(--np-glide) var(--ease-in-out),
      flex-basis var(--np-glide) var(--ease-in-out),
      min-width var(--np-glide) var(--ease-in-out);
  }
  .meta {
    flex: 0 1 462px;
    min-width: 260px;
    min-height: 0;
    /* Full height in both modes with centered content, so switching tabs never
       repositions the details/list; in lyrics mode the flexing lyrics slot
       fills the box and the centering is a no-op. */
    align-self: stretch;
    display: flex;
    flex-direction: column;
    justify-content: center;
    transition:
      flex-grow var(--np-glide) var(--ease-in-out),
      flex-shrink var(--np-glide) var(--ease-in-out),
      flex-basis var(--np-glide) var(--ease-in-out),
      min-width var(--np-glide) var(--ease-in-out);
  }
  /* Flex items do not collapse margins, so keep the wrapper a flex column to
     preserve the original details-to-divider spacing. */
  .meta-details {
    display: flex;
    flex-direction: column;
  }
  .stage-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    width: 100%;
    min-width: 0;
    max-width: 100%;
  }
  .stage-details {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    max-width: 100%;
  }
  .stage-details .title {
    margin-bottom: 4px;
  }
  .stage-details .badges {
    justify-content: center;
    margin-bottom: 0;
  }
  .spine {
    display: flex;
    flex-direction: column;
    gap: 16px;
    align-self: center;
  }
  .spine button {
    writing-mode: vertical-rl;
    padding: 2px 0 2px 6px;
    border: none;
    border-left: 1px solid transparent;
    background: none;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-muted);
    cursor: pointer;
  }
  .spine button.active {
    color: var(--text);
    border-left-color: var(--accent);
  }
  .spine button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 2px;
  }
  .now-playing.with-lyrics {
    /* The composition widens and the stage keeps most of its share so the
       cover only steps left; the details sit under it and the lyrics take the
       column. The cover cap is shared with Next Up, so it never resizes. */
    max-width: 1800px;
    gap: 96px;
  }
  .now-playing.with-lyrics .stage {
    flex: 0 0 62%;
  }
  /* Narrow windows: keep a usable lyrics column instead of squeezing it. */
  @media (max-width: 1200px) {
    .now-playing.with-lyrics {
      gap: 48px;
    }
    .now-playing.with-lyrics .stage {
      flex: 0 0 54%;
    }
  }
  .now-playing.with-lyrics .meta {
    flex: 1 1 0%;
    min-width: 0;
  }
  .lyrics-slot {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .loading {
    margin: 0;
  }
</style>
