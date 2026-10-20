<script lang="ts">
  import { untrack } from "svelte";
  import { flip } from "svelte/animate";
  import { cubicInOut, cubicOut } from "svelte/easing";
  import { fade, fly, scale, slide, type TransitionConfig } from "svelte/transition";
  import { api, type QueueItem } from "../lib/api";
  import NameLink from "../lib/components/NameLink.svelte";
  import QueueArtistLinks from "../lib/components/QueueArtistLinks.svelte";
  import { albumRoute } from "../lib/queueLinks";
  import { appearance } from "../lib/appearance.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import DiscArt from "../lib/components/DiscArt.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import LyricsPanel from "../lib/components/LyricsPanel.svelte";
  import { dspBadgeTitle } from "../lib/dsp";
  import { formatDuration } from "../lib/format";
  import { lyrics } from "../lib/lyrics.svelte";
  import { formatQuality, type QualityDisplay } from "../lib/quality";
  import { scrobbled } from "../lib/scrobbled.svelte";
  import type { Nav, Route } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  import { tiltFromPointer } from "../lib/tilt";

  /// Cover morph (open and close): the artwork travels between the player
  /// bar's thumbnail and the stage, so Now Playing reads as the bar's cover
  /// opening up rather than a page swap. Same curve both ways.
  const MORPH_OPEN_MS = 480;
  const MORPH_CLOSE_MS = 360;
  const MORPH_EASE = "cubic-bezier(0.2, 0, 0, 1)";

  function thumbRect(): DOMRect | null {
    const rect = document.querySelector<HTMLElement>(".player .thumb")?.getBoundingClientRect();
    return rect && rect.width > 0 ? rect : null;
  }

  /// The transform that lays `art` (at rest) over `thumb`.
  function towardThumb(art: DOMRect, thumb: DOMRect): string {
    const dx = thumb.left + thumb.width / 2 - (art.left + art.width / 2);
    const dy = thumb.top + thumb.height / 2 - (art.top + art.height / 2);
    return `translate(${dx}px, ${dy}px) scale(${thumb.width / art.width})`;
  }

  /// Outro pins the closing view (fixed, at its on-screen box) over the
  /// revealed route, so the swap dissolves in place instead of reflowing
  /// .main mid-fade; fixed rather than absolute because the route underneath
  /// gets its scroll offset back in this same update. The artwork flies back
  /// into the bar's thumbnail while the text columns sink and fade.
  function npOut(node: HTMLElement): TransitionConfig {
    const box = node.getBoundingClientRect();
    const art = node.querySelector<HTMLElement>(".art");
    const artBox = art?.getBoundingClientRect();
    Object.assign(node.style, {
      position: "fixed",
      left: `${box.left}px`,
      top: `${box.top}px`,
      width: `${box.width}px`,
      height: `${box.height}px`,
      margin: "0",
      zIndex: "1",
      pointerEvents: "none",
    });
    if (appearance.reducedMotion) {
      return { duration: 150, css: (t) => `opacity: ${t}` };
    }
    const thumb = thumbRect();
    if (!art || !artBox || artBox.width === 0 || !thumb) {
      return {
        duration: 220,
        easing: cubicOut,
        css: (t) => `opacity: ${t}; transform: translateY(${(1 - t) * 10}px) scale(${0.99 + 0.01 * t})`,
      };
    }
    art.animate(
      [
        { transform: "none", opacity: 1 },
        { opacity: 1, offset: 0.7 },
        { transform: towardThumb(artBox, thumb), opacity: 0 },
      ],
      { duration: MORPH_CLOSE_MS, easing: MORPH_EASE, fill: "forwards" },
    );
    for (const el of node.querySelectorAll<HTMLElement>(".spine, .stage-details, .meta")) {
      el.animate(
        [
          { opacity: 1, transform: "none" },
          { opacity: 0, transform: "translateY(10px)" },
        ],
        { duration: 200, easing: "cubic-bezier(0.4, 0, 1, 1)", fill: "forwards" },
      );
    }
    // No css: the pieces animate themselves; this only times the removal.
    return { duration: MORPH_CLOSE_MS };
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
    // The engine's position, not a track-id search: a track queued twice
    // must not make Next up restart from its first copy.
    const index = player.state.current_index ?? -1;
    if (index < 0 || player.state.queue[index]?.track_id !== current.track_id) return [];
    return player.state.queue.slice(index + 1, index + 4);
  });

  /// Lyrics belonging to the track on screen; null while the store is still
  /// catching up (a skip clears the previous track's lyrics synchronously in
  /// the effect below, one render after the track changes).
  const currentLyrics = $derived(
    track !== null && lyrics.trackId === track.track_id ? lyrics.lyrics : null,
  );
  /// The store has not caught up with the track yet, or a fetch is pending.
  const awaitingLyrics = $derived(
    track !== null && (lyrics.trackId !== track.track_id || lyrics.loading),
  );
  /// While lyrics are pending the lyrics layout stays put with the loading
  /// placeholder, so the Next Up column never flashes in between. A settled
  /// miss (trackId matches, no lyrics, not loading) falls back to Next Up.
  const showLyrics = $derived(lyrics.tab === "lyrics" && (awaitingLyrics || currentLyrics !== null));
  const hasLyrics = $derived(awaitingLyrics || currentLyrics !== null);
  /// The spine layout (cover + side column) belongs to lyrics.
  const wide = $derived(showLyrics);
  /// Display-only tab: a settled no-lyrics miss shows the Next up fallback, so
  /// the Lyrics tab must not claim selection over an absent panel. The store
  /// keeps the user's choice, so a track with lyrics snaps back to Lyrics.
  const effectiveTab = $derived(
    track !== null && lyrics.tab === "lyrics" && !hasLyrics ? "next" : lyrics.tab,
  );

  const NP_TABS = ["lyrics", "next"] as const;

  /// Vertical tablist keyboard pattern (spec §6.2): roving focus with
  /// Up/Down (wrapping) and Home/End; activation follows focus.
  function onTabKeydown(event: KeyboardEvent, current: (typeof NP_TABS)[number]) {
    const index = NP_TABS.indexOf(current);
    let target: (typeof NP_TABS)[number] | null = null;
    if (event.key === "ArrowDown") target = NP_TABS[(index + 1) % NP_TABS.length];
    else if (event.key === "ArrowUp") target = NP_TABS[(index - 1 + NP_TABS.length) % NP_TABS.length];
    else if (event.key === "Home") target = NP_TABS[0];
    else if (event.key === "End") target = NP_TABS[NP_TABS.length - 1];
    if (!target) return;
    event.preventDefault();
    lyrics.setTab(target);
    document.getElementById(`np-tab-${target}`)?.focus();
  }

  /// Transition duration that collapses to 0 when reduced motion is requested.
  const dur = (ms: number) => (appearance.reducedMotion ? 0 : ms);

  /// Entrance: the artwork morphs up out of the bar's thumbnail (or, with no
  /// thumbnail to come from, scales in), then the tabs and the details/list
  /// rise in after it. `entering` scopes the stagger to the mount, so later
  /// tab and track swaps keep their own transitions.
  // svelte-ignore state_referenced_locally
  const morph = !appearance.reducedMotion && player.state.current !== null && thumbRect() !== null;
  let entering = $state(!appearance.reducedMotion);
  let artEl = $state<HTMLElement | null>(null);
  $effect(() => {
    const timer = setTimeout(() => (entering = false), 900);
    untrack(() => {
      const thumb = thumbRect();
      const box = artEl?.getBoundingClientRect();
      if (!morph || !artEl || !thumb || !box || box.width === 0) return;
      artEl.animate([{ transform: towardThumb(box, thumb) }, { transform: "none" }], {
        duration: MORPH_OPEN_MS,
        easing: MORPH_EASE,
      });
    });
    return () => clearTimeout(timer);
  });

  /// Sleeve hover tilt (max degrees) and whether the pointer is on it. The
  /// pointer geometry is written straight to the art wrapper's custom
  /// properties so pointermove stays off the rune path; the class only gates
  /// the tracking transition. Listeners live on the rectangular wrapper, not
  /// the rounded button, so slipping over a corner never resets the tilt.
  /// Mouse only, and off under reduced motion (the stylesheet hides the glare
  /// as a backstop).
  const MAX_TILT = 8;
  /// First-contact window: the initial lean eases in on the slow return
  /// curve, then tracking hands over to the tight linear follow.
  const ENTER_MS = 480;
  let sleeveHover = $state(false);
  let sleeveEntering = $state(false);
  let enterTimer: number | undefined;

  function sleeveEnter(e: PointerEvent) {
    if (appearance.coverShape !== "sleeve" || e.pointerType !== "mouse" || appearance.reducedMotion) return;
    window.clearTimeout(enterTimer);
    sleeveEntering = true;
    enterTimer = window.setTimeout(() => (sleeveEntering = false), ENTER_MS);
  }

  function sleeveMove(e: PointerEvent) {
    if (appearance.coverShape !== "sleeve" || e.pointerType !== "mouse" || appearance.reducedMotion) return;
    const el = e.currentTarget as HTMLElement;
    const t = tiltFromPointer(el.getBoundingClientRect(), e.clientX, e.clientY, MAX_TILT);
    el.style.setProperty("--rx", `${t.rx}deg`);
    el.style.setProperty("--ry", `${t.ry}deg`);
    el.style.setProperty("--tilt-gx", `${t.gx}%`);
    el.style.setProperty("--tilt-gy", `${t.gy}%`);
    el.style.setProperty("--tilt-glare", `${t.glare}`);
    sleeveHover = true;
  }

  function sleeveLeave(e: PointerEvent) {
    window.clearTimeout(enterTimer);
    sleeveEntering = false;
    sleeveHover = false;
    const el = e.currentTarget as HTMLElement;
    el.style.removeProperty("--rx");
    el.style.removeProperty("--ry");
    el.style.removeProperty("--tilt-gx");
    el.style.removeProperty("--tilt-gy");
    el.style.removeProperty("--tilt-glare");
  }

  /// The lyrics panel mounts as the layout morph settles — laying lines out
  /// mid-glide continuously re-wraps them and drifts the centered line.
  /// GLIDE_MS is 460ms of the 520ms --np-glide: the ease-in-out tail is flat,
  /// so the column is within a few px of final by then. A flip back within a
  /// moment (cached track change) never moved the layout, so it mounts at once.
  const GLIDE_MS = 460;
  let lyricsSettled = $state(false);
  let settleSeen = false;
  let hiddenSince = 0;
  $effect(() => {
    const shown = showLyrics;
    const now = performance.now();
    if (!settleSeen) {
      settleSeen = true;
      lyricsSettled = shown;
      if (!shown) hiddenSince = now;
      return;
    }
    if (!shown) {
      lyricsSettled = false;
      hiddenSince = now;
      return;
    }
    if (now - hiddenSince < 160) {
      lyricsSettled = true;
      return;
    }
    const timer = setTimeout(() => (lyricsSettled = true), dur(GLIDE_MS));
    return () => clearTimeout(timer);
  });

  $effect(() => {
    void lyrics.open(track);
  });

  /// Name links close Now Playing so the page they open is visible.
  function go(route: Route) {
    nav.closeNowPlaying();
    nav.goto(route);
  }

  async function openAlbum(item: QueueItem) {
    const route = await albumRoute(item, api.getTrack);
    if (route) go(route);
  }
</script>

{#snippet details(t: QueueItem)}
  <span class="eyebrow">Now playing</span>
  <h2 class="title">{t.title}</h2>
  <p class="byline"><QueueArtistLinks item={t} {go} />{#if t.album}{" — "}<NameLink label={t.album} onOpen={() => void openAlbum(t)} />{/if}</p>
  <div class="badges">
    {#if quality}<span class="badge" class:max={quality === "MAX"}>{quality}</span>{/if}
    {#if player.state.bit_perfect}<span class="badge">Bit-perfect</span>{:else if player.state.dsp_active}<span class="badge" title={dspBadgeTitle(player.state.dsp_stages)}>DSP</span>{/if}
    {#if qualityDetail}<span class="badge muted">{qualityDetail}</span>{/if}
    {#if scrobbled.trackId === t.track_id}<span class="badge">Scrobbled</span>{/if}
  </div>
{/snippet}

<div
  class="now-playing"
  class:with-lyrics={wide}
  class:has-lyrics={hasLyrics}
  class:morph
  class:entering
  out:npOut
>
  <div class="stage">
    {#if track}
      <div class="stage-row">
        <div
          class="art"
          bind:this={artEl}
          role="presentation"
          onpointerenter={sleeveEnter}
          onpointermove={sleeveMove}
          onpointerleave={sleeveLeave}
        >
          <!-- Keyed on the track so a track change scale-dissolves the artwork
               instead of snapping; the grid stack lets outgoing and incoming
               overlap in the same cell during the swap. The transform lives on
               this wrapper, so the disc's inner rotation never conflicts. -->
          {#key track.track_id}
            <div
              class="art-stack"
              in:scale={{ start: 0.96, duration: dur(260), easing: cubicOut }}
              out:scale={{ start: 0.98, duration: dur(200), easing: cubicOut }}
            >
              {#if appearance.coverShape === "disc"}
                <button class="art-disc" onclick={() => nav.closeNowPlaying()} aria-label="Close now playing">
                  <DiscArt uuid={track.cover} playing={player.state.playing} />
                </button>
              {:else}
                <button
                  class="art-sleeve lit"
                  class:hovering={sleeveHover}
                  class:entering={sleeveEntering}
                  onclick={() => nav.closeNowPlaying()}
                  aria-label="Close now playing"
                >
                  <Cover uuid={track.cover} size={640} radius={12} alt={`${track.title} artwork`} />
                  <span class="art-glare" aria-hidden="true"></span>
                </button>
              {/if}
            </div>
          {/key}
        </div>
        {#if track}
          <div
            class="spine"
            role="tablist"
            aria-orientation="vertical"
            aria-label="Now playing content"
            in:fade={{ duration: dur(200), easing: cubicOut }}
            out:fade={{ duration: dur(160) }}
          >
            <button
              id="np-tab-lyrics"
              role="tab"
              tabindex={effectiveTab === "lyrics" ? 0 : -1}
              aria-selected={effectiveTab === "lyrics"}
              aria-controls="np-panel-lyrics"
              class:active={effectiveTab === "lyrics"}
              onclick={() => lyrics.setTab("lyrics")}
              onkeydown={(event) => onTabKeydown(event, "lyrics")}
            >
              Lyrics
            </button>
            <button
              id="np-tab-next"
              role="tab"
              tabindex={effectiveTab === "next" ? 0 : -1}
              aria-selected={effectiveTab === "next"}
              aria-controls="np-panel-next"
              class:active={effectiveTab === "next"}
              onclick={() => lyrics.setTab("next")}
              onkeydown={(event) => onTabKeydown(event, "next")}
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
            {#key track.track_id}
              <div class="details-swap" in:fly={{ y: 6, duration: dur(200), easing: cubicOut }}>
                {@render details(track)}
              </div>
            {/key}
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
        {@const shown = currentLyrics}
        <div
          class="lyrics-slot"
          role="tabpanel"
          id="np-panel-lyrics"
          aria-labelledby="np-tab-lyrics"
          in:fly={{ y: 8, duration: dur(320), delay: dur(80), easing: cubicOut }}
          out:fly={{ y: 12, duration: dur(280), easing: cubicInOut }}
        >
          {#if shown}
            {#if lyricsSettled}
              {#key shown.track_id}
                <LyricsPanel {player} dto={shown} />
              {/key}
            {/if}
          {:else}
            <p class="muted loading">Loading lyrics…</p>
          {/if}
        </div>
      {:else}
        <div
          class="next-slot"
          role="tabpanel"
          id="np-panel-next"
          aria-labelledby="np-tab-next"
        >
          <div
            class="meta-details"
            in:fly={{ y: -16, duration: dur(260), delay: dur(40), easing: cubicOut }}
            out:fly={{ y: -10, duration: dur(420), easing: cubicInOut }}
          >
            {#key track.track_id}
              <div class="details-swap" in:fly={{ y: 6, duration: dur(200), easing: cubicOut }}>
                {@render details(track)}
              </div>
            {/key}
          </div>

          <hr
            in:fly={{ y: -16, duration: dur(240), delay: dur(60), easing: cubicOut }}
            out:fly={{ y: -10, duration: dur(420), easing: cubicInOut }}
          />

          <div
            class="meta-list"
            in:fly={{ y: -16, duration: dur(240), delay: dur(80), easing: cubicOut }}
            out:fly={{ y: -10, duration: dur(420), easing: cubicInOut }}
          >
            {#if nextUp.length > 0}
              <ul class="next-up">
                {#each nextUp as item (item.track_id)}
                  <!-- Queue advance: the finished top row fades in place (Svelte
                       fixes it out of flow), the rest glide up via FLIP, and the
                       incoming row rises in. The outro stays transform-free so it
                       cannot override Svelte's compensating translate. -->
                  <li
                    in:fly={{ y: 10, duration: dur(240), easing: cubicOut }}
                    out:fade={{ duration: dur(160) }}
                    animate:flip={{ duration: dur(240), easing: cubicOut }}
                  >
                    <span class="thumb"><Cover uuid={item.cover} size={80} radius={6} alt="" /></span>
                    <span class="next-title"><NameLink label={item.title} onOpen={() => void openAlbum(item)} /></span>
                    <span class="muted next-artist"><QueueArtistLinks {item} {go} /></span>
                    <span class="muted tnum">{formatDuration(item.duration_secs)}</span>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="muted">Queue is empty</p>
            {/if}
          </div>
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
    --np-glide: 520ms;
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
    /* Entrance: the view rises, the artwork scales up, then the tabs and
       details stagger in (`.entering`). With the cover morph the artwork
       flies in from the bar instead and the view itself holds still, since
       fading it would fade the morphing cover too. Closing is npOut. */
    animation: np-in var(--dur-view) var(--ease-enter) both;
  }
  .now-playing.morph {
    animation: none;
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
    /* Over the player bar and the fading sidebar: the morph starts and ends
       on the bar's thumbnail. */
    position: relative;
    z-index: 1;
    animation: np-art var(--dur-view) var(--ease-enter) 60ms both;
  }
  .now-playing.morph .art {
    animation: none;
  }
  @keyframes np-art {
    from { opacity: 0; transform: scale(0.95) translateY(8px); }
  }
  /* Mount stagger, after the cover: tabs, details, rule, then the Next up
     rows one by one (or the lyrics column). */
  .now-playing.entering :is(.spine, .stage-details, .lyrics-slot, .meta-details, hr, .next-up li, .meta > .eyebrow, .meta > p) {
    animation: np-rise 380ms var(--ease-enter) backwards;
    animation-delay: 140ms;
  }
  .now-playing.entering .spine { animation-delay: 220ms; }
  .now-playing.entering hr { animation-delay: 180ms; }
  .now-playing.entering .next-up li:nth-child(1) { animation-delay: 220ms; }
  .now-playing.entering .next-up li:nth-child(2) { animation-delay: 260ms; }
  .now-playing.entering .next-up li:nth-child(3) { animation-delay: 300ms; }
  @keyframes np-rise {
    from { opacity: 0; transform: translateY(calc(var(--rise) * 1.2)); }
  }
  /* Track-change crossfade: key swaps stack in the same grid cell so the
     outgoing art fades under the incoming instead of reflowing the stage. */
  .art-stack {
    grid-area: 1 / 1;
    width: 100%;
    /* Depth for the sleeve's hover tilt (the disc ignores it). Kept shallow
       so the far edge reads as tilt, not a bulge toward the viewer. */
    perspective: 1400px;
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
    .art-sleeve {
      transform: none;
    }
    .art-glare {
      display: none;
    }
  }
  /* Both shapes are one close affordance (spec §7.2): clicking the artwork
     returns, disc or sleeve. The disc button stays transparent (no lit edge).
     The hover lift signals the affordance; the disc's spin lives on the inner
     .disc, so the lift never fights the rotation. */
  .art-sleeve,
  .art-disc {
    display: block;
    width: 100%;
    margin: 0 auto;
    transition: transform var(--dur-state) var(--ease-in-out);
  }
  .art-disc:hover {
    transform: scale(1.015);
  }
  /* :not(:disabled) is required to out-specify app.css's global press rule. */
  .art-disc:not(:disabled):active {
    transform: scale(0.99);
  }
  /* Sleeve (square): the same close affordance, but it reads as a record box
     under the cursor — a perspective tilt toward the pointer with a specular
     glare mirrored through the center (no grow; the tilt alone carries it).
     The press lives in --tilt-scale so it composes with the tilt instead of
     replacing it. */
  .art-sleeve {
    position: relative;
    border-radius: var(--radius-card);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.45);
    --tilt-scale: 1;
    /* The leave settle: slower than the hover tracking, decelerating into
       flat (var(--ease-enter)'s fast-start curve) so the return never snaps. */
    --tilt-return: 480ms;
    transform: rotateX(var(--rx, 0deg)) rotateY(var(--ry, 0deg)) scale(var(--tilt-scale));
    transition-duration: var(--tilt-return);
    transition-timing-function: var(--ease-enter);
    will-change: transform;
  }
  .art-sleeve.hovering {
    /* Track the pointer tightly; the base rule eases flat on leave. */
    transition-duration: var(--dur-micro);
    transition-timing-function: linear;
  }
  /* First contact: lean in on the slow settle curve, then hand over to the
     tight tracking above. Declared after .hovering so it wins while active. */
  .art-sleeve.entering {
    transition-duration: var(--tilt-return);
    transition-timing-function: var(--ease-enter);
  }
  .art-sleeve:not(:disabled):active {
    --tilt-scale: 0.99;
  }
  .art-glare {
    position: absolute;
    inset: 0;
    pointer-events: none;
    border-radius: var(--radius-card);
    opacity: var(--tilt-glare, 0);
    background: radial-gradient(
      circle at var(--tilt-gx, 50%) var(--tilt-gy, 50%),
      rgba(255, 255, 255, var(--glare-alpha)),
      rgba(255, 255, 255, 0) 40%
    );
    mix-blend-mode: screen;
    transition:
      --tilt-gx var(--tilt-return) var(--ease-enter),
      --tilt-gy var(--tilt-return) var(--ease-enter),
      opacity var(--tilt-return) var(--ease-enter);
  }
  .art-sleeve.hovering .art-glare {
    transition-duration: var(--dur-micro);
    transition-timing-function: linear;
  }
  .art-sleeve.entering .art-glare {
    transition-duration: var(--tilt-return);
    transition-timing-function: var(--ease-enter);
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
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
    margin: 0 0 6px;
  }
  .byline {
    font-size: 15.5px;
    font-weight: 500;
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
  .byline :global(.name-link:hover),
  .next-artist :global(.name-link:hover) {
    color: var(--text);
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
    gap: 24px;
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
    /* Full height in both modes with centered content. The lyrics slot
       overlays absolutely (below), so it never participates in the centering:
       the details/list hold their position while the tab swap's transitions
       overlap. */
    position: relative;
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
  /* Structural tabpanel only: display: contents keeps the details, divider,
     and list in .meta's flex flow (with their transitions) exactly as the
     unwrapped layout had them. */
  .next-slot {
    display: contents;
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
    font-size: 34px;
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
    /* The shorthand replaces app.css's global button transition, so the
       press-scale transform is listed here to keep it animating. */
    transition:
      color var(--dur-state) var(--ease-in-out),
      border-color var(--dur-state) var(--ease-in-out),
      transform var(--dur-micro) var(--ease-enter);
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
    /* Out of flow so the incoming slot never displaces the centered
       details/list while their out transitions still hold them in the DOM. */
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .loading {
    margin: 0;
  }
</style>
