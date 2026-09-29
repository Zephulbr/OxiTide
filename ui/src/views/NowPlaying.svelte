<script lang="ts">
  import type { TransitionConfig } from "svelte/transition";
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
    node.style.inset = "16px 20px"; // matches .main padding in app.css
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

  $effect(() => {
    void lyrics.open(track);
  });
</script>

<div class="now-playing" class:with-lyrics={lyrics.tab === "lyrics" && lyrics.lyrics !== null} out:npOut>
  <div class="art">
    {#if track}
      {#if appearance.coverShape === "disc"}
        <button class="art-disc" onclick={() => nav.closeNowPlaying()} aria-label="Close now playing">
          <DiscArt uuid={track.cover} playing={player.state.playing} />
        </button>
      {:else}
        <button class="art-sleeve lit" onclick={() => nav.closeNowPlaying()} aria-label="Close now playing">
          <Cover uuid={track.cover} size={640} radius={12} alt={`${track.title} artwork`} />
        </button>
      {/if}
    {:else}
      <div class="art-empty lit"><Icon name="music" size={64} /></div>
    {/if}
  </div>

  <div class="meta">
    {#if track}
      <span class="eyebrow">Now playing</span>
      <h2 class="title">{track.title}</h2>
      <p class="byline">{track.artist}{track.album ? ` — ${track.album}` : ""}</p>
      <div class="badges">
        {#if quality}<span class="badge" class:max={quality === "MAX"}>{quality}</span>{/if}
        {#if player.state.bit_perfect}<span class="badge">Bit-perfect</span>{/if}
        {#if qualityDetail}<span class="badge muted">{qualityDetail}</span>{/if}
      </div>

      <hr />

      {#if lyrics.loading}
        <p class="muted loading">Loading lyrics…</p>
      {:else}
        {#if lyrics.lyrics}
          <div class="tabs">
            <button class:active={lyrics.tab === "lyrics"} onclick={() => lyrics.setTab("lyrics")}>
              Lyrics
            </button>
            <button class:active={lyrics.tab === "next"} onclick={() => lyrics.setTab("next")}>
              Next up
            </button>
          </div>
        {:else}
          <span class="eyebrow">Next up</span>
        {/if}

        {#if lyrics.tab === "lyrics" && lyrics.lyrics}
          {@const shown = lyrics.lyrics}
          {#key shown.track_id}
            <LyricsPanel {player} dto={shown} />
          {/key}
        {:else}
          {#if nextUp.length > 0}
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
        {/if}
      {/if}
    {:else}
      <span class="eyebrow">Now playing</span>
      <p class="muted">Nothing playing</p>
    {/if}
  </div>
</div>

<style>
  .now-playing {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(260px, 420px);
    gap: 32px;
    height: 100%;
    align-items: center;
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
    animation: np-art var(--dur-view) var(--ease-in) 60ms both;
  }
  @keyframes np-art {
    from { opacity: 0; transform: scale(0.92); }
  }
  .meta > * {
    animation: np-rise var(--dur-view) var(--ease-in) both;
  }
  .meta > *:nth-child(1) { animation-delay: 90ms; }
  .meta > *:nth-child(2) { animation-delay: 120ms; }
  .meta > *:nth-child(3) { animation-delay: 150ms; }
  .meta > *:nth-child(4) { animation-delay: 180ms; }
  .meta > *:nth-child(5) { animation-delay: 210ms; }
  .meta > *:nth-child(6) { animation-delay: 240ms; }
  .meta > *:nth-child(7) { animation-delay: 270ms; }
  .meta > *:nth-child(8) { animation-delay: 300ms; }
  @keyframes np-rise {
    from { opacity: 0; transform: translateY(10px); }
  }
  @keyframes np-fade {
    from { opacity: 0; }
  }
  /* Reduced motion (spec §12): slides become fades, no stagger. */
  @media (prefers-reduced-motion: reduce) {
    .now-playing,
    .art,
    .meta > * {
      animation-name: np-fade;
      animation-delay: 0ms;
    }
  }
  /* Both shapes are one close affordance (spec §7.2): clicking the artwork
     returns, disc or sleeve. The disc button stays transparent (no lit edge). */
  .art-sleeve,
  .art-disc {
    display: block;
    width: min(100%, 55vh);
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
    font-size: 28px;
    font-weight: 640;
    letter-spacing: -0.02em;
    margin: 0 0 6px;
  }
  .byline {
    font-family: var(--font-display);
    font-style: italic;
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
    grid-template-columns: var(--thumb-size) 1fr auto;
    grid-template-rows: auto auto;
    column-gap: 10px;
    align-items: center;
  }
  .thumb {
    grid-row: 1 / span 2;
    width: var(--thumb-size);
  }
  .next-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .next-artist {
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .next-up .tnum {
    grid-row: 1 / span 2;
  }
  .now-playing.with-lyrics {
    align-items: start;
  }
  .now-playing.with-lyrics .art {
    align-self: center;
  }
  .now-playing.with-lyrics .meta {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .tabs {
    display: flex;
    gap: 14px;
    margin-bottom: 10px;
  }
  .tabs button {
    padding: 0 0 3px;
    border: none;
    border-bottom: 1px solid transparent;
    background: none;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-muted);
    cursor: pointer;
  }
  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
  .tabs button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 2px;
  }
  .loading {
    margin: 0;
  }
</style>
