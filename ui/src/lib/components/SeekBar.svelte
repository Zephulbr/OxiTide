<script lang="ts">
  import { appearance } from "../appearance.svelte";
  import { formatDuration } from "../format";
  import type { Player } from "../player.svelte";
  import { seekByKey, seekRatio } from "../seek";
  import { waveFillPath, wavePath, type WaveTaper } from "../wave";

  let {
    player,
    variant = "edge",
    taper = false,
  }: {
    player: Player;
    variant?: "edge" | "inline";
    /// Floating player bar: the wave lies flat on the edge past the rounded
    /// corners (where the rim hands over to it) before it rises.
    taper?: boolean;
  } = $props();

  /// Mirrors the CSS below: the rim's corner (--radius-sheet) plus its
  /// cross-fade with the wave (--corner-fade); the wave rises after that.
  const TAPER_FLAT_PX = 16 + 12;
  const TAPER_RAMP_PX = 32;

  let width = $state(0);
  let seekEl = $state<HTMLElement | null>(null);
  // Measured only while tapering: the docked bar needs no observer.
  $effect(() => {
    if (!taper || !seekEl) return;
    const observer = new ResizeObserver(([entry]) => (width = entry.contentRect.width));
    observer.observe(seekEl);
    return () => observer.disconnect();
  });
  const waveTaper = $derived<WaveTaper | undefined>(
    taper && variant === "edge" && width > 0
      ? { flat: TAPER_FLAT_PX / width, ramp: TAPER_RAMP_PX / width }
      : undefined,
  );
  /// A finer path when tapered, so the short ramps stay smooth.
  const waveCount = $derived(waveTaper ? 192 : 96);

  let dragging = $state(false);
  let dragValue = $state(0);

  const shown = $derived(player.pendingSeek ?? player.state.position_ms);
  const position = $derived(dragging ? dragValue : shown);
  const duration = $derived(player.state.duration_ms);
  const progress = $derived((position / Math.max(duration, 1)) * 100);
  const valueText = $derived(
    `${formatDuration(position / 1000)} of ${formatDuration(Math.max(duration, 0) / 1000)}`,
  );

  function seekPointer(event: PointerEvent) {
    const el = event.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    const ratio = (clientX: number) => seekRatio(clientX, rect.left, rect.width);
    dragging = true;
    dragValue = ratio(event.clientX) * duration;
    el.setPointerCapture(event.pointerId);
    const onMove = (e: PointerEvent) => {
      dragValue = ratio(e.clientX) * duration;
    };
    const onUp = (e: PointerEvent) => {
      dragging = false;
      el.removeEventListener("pointermove", onMove);
      el.removeEventListener("pointerup", onUp);
      player.seek(Math.round(ratio(e.clientX) * duration));
    };
    el.addEventListener("pointermove", onMove);
    el.addEventListener("pointerup", onUp);
  }

  function seekKeydown(event: KeyboardEvent) {
    const target = seekByKey(event.key, position, duration);
    if (target === null) return;
    player.seek(target);
    event.preventDefault();
  }
</script>

{#if player.state.current}
  {@const current = player.state.current}
  <div
    class="seek {variant}"
    class:wave={appearance.waveSeek}
    class:dragging
    data-oneko-perch
    role="slider"
    tabindex="0"
    aria-label="Playback position"
    aria-valuemin={0}
    aria-valuemax={Math.max(duration, 1)}
    aria-valuenow={Math.round(position)}
    aria-valuetext={valueText}
    onpointerdown={seekPointer}
    onkeydown={seekKeydown}
    bind:this={seekEl}
  >
    {#if variant === "edge"}
      <div class="seek-rim" aria-hidden="true">
        <div class="rim-track"></div>
        <div class="rim-fill" style:clip-path="inset(0 {100 - progress}% 0 0)"></div>
      </div>
    {/if}
    {#if appearance.waveSeek}
      {@const pct = (position / Math.max(duration, 1)) * 100}
      {@const d = wavePath(current.track_id, 800, 14, waveCount, waveTaper)}
      {#if variant === "edge"}
        <svg width="0" height="0" aria-hidden="true">
          <defs>
            <clipPath id="seek-cap-clip" clipPathUnits="objectBoundingBox">
              <path d={waveFillPath(current.track_id, waveCount, waveTaper)} />
            </clipPath>
          </defs>
        </svg>
        <div class="seek-flat" aria-hidden="true">
          <div class="seek-fill" style:width="{progress}%"></div>
        </div>
        <div class="seek-cap" aria-hidden="true"></div>
      {/if}
      <div class="seek-wave">
        <svg viewBox="0 0 800 14" preserveAspectRatio="none" aria-hidden="true">
          <path class="wave-base" d={d} fill="none" stroke="var(--line)" stroke-width="1.5" vector-effect="non-scaling-stroke" />
          <clipPath id="seek-played"><rect x="0" y="0" width="{pct}%" height="100%" /></clipPath>
          <path d={d} fill="none" stroke="var(--accent)" stroke-width="1.5" vector-effect="non-scaling-stroke" clip-path="url(#seek-played)" />
        </svg>
      </div>
    {:else}
      <div class="seek-line">
        <div class="seek-fill" style:width="{progress}%"></div>
        <div class="seek-thumb" style:left="{progress}%"></div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .seek {
    cursor: pointer;
    display: flex;
    align-items: center;
  }
  /* Edge: the player bar's top edge — the unplayed portion doubles as the
     separator; rests as a straight line, grows on hover/focus/drag. */
  /* The box is a 24px hit target (AC-5); the visible line/wave stays at its
     top (`flex-start`) and the rest is transparent. */
  .seek.edge {
    position: absolute;
    left: 0;
    right: 0;
    top: -1px;
    height: 24px;
    align-items: flex-start;
  }
  /* Wave branch: the wave box hangs above the bar's edge (its dips overlap the
     frosted edge by 1px) so the content behind stays visible above the wave. */
  .seek.edge.wave {
    top: -9px;
    height: 24px;
  }
  /* Inline (mini): a visible control at rest with a full 24px hit height. */
  .seek.inline {
    width: 100%;
    height: 24px;
  }
  .seek-line {
    position: relative;
    width: 100%;
    height: 4px;
    background: var(--line);
    transition: height var(--dur-micro) var(--ease-enter);
  }
  .seek.edge .seek-line {
    height: 3px;
  }
  .seek-fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--accent);
  }
  .seek-thumb {
    position: absolute;
    top: 50%;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
    transform: translate(-50%, -50%) scale(0);
    transition: transform var(--dur-micro) var(--ease-enter);
  }
  .seek.edge:hover .seek-line,
  .seek.edge:focus-visible .seek-line,
  .seek.edge.dragging .seek-line {
    height: 5px;
  }
  .seek.edge:hover .seek-thumb,
  .seek.edge:focus-visible .seek-thumb,
  .seek.edge.dragging .seek-thumb,
  .seek.inline .seek-thumb {
    transform: translate(-50%, -50%) scale(1);
  }
  /* Wave mode at rest: a completely straight progress line (same recipe as
     the line variant), straddling the bar edge; it fades out as the wave
     inflates from that same edge on hover/focus/drag. */
  .seek-flat {
    position: absolute;
    left: 0;
    right: 0;
    top: 8px;
    height: 2px;
    background: var(--line);
    transition: opacity var(--dur-reveal) var(--ease-enter);
  }
  .seek.edge:hover .seek-flat,
  .seek.edge:focus-visible .seek-flat,
  .seek.edge.dragging .seek-flat {
    opacity: 0;
  }
  /* Wave cap (edge only): fills the area under the stroke up to the bar edge,
     so the wave doubles as the bar's shaped top edge. It uses the bar's own
     glass recipe (the veil's top-edge tint, --player-veil-top, over a blur of
     whatever is behind) so it matches the bar by construction — including
     content scrolling behind the floating bar, which the old opaque
     scrimmed-wash fill could not track. In Instrument --glass-blur is 0 and
     the veil is the opaque --surface-2, so the cap is a plain opaque strip
     matching the opaque bar. The clip is the
     wave's objectBoundingBox shape (its 12.6-unit space matches the stroke's
     10/14 scale, so the fill edge stays on the wave centerline). The hover
     geometry is pre-baked at 1.2x (offsets are relative to .seek, whose top
     is -9px and whose bar edge is 9px down: 9 - 10.8 = -1.8) and the grow is
     a plain scaleY, keeping the fill and the stroke in lockstep. The box's
     bottom edge sits exactly on the bar edge (the transform anchor, shared
     with the stroke at 50% 90%). Collapsed at rest: the flat line carries the
     progress until hover. */
  .seek-cap {
    position: absolute;
    left: 0;
    right: 0;
    top: -1.8px;
    height: 10.8px;
    background: var(--player-veil-top);
    backdrop-filter: blur(var(--glass-blur));
    clip-path: url(#seek-cap-clip);
    transform-origin: bottom;
    transition: transform var(--dur-reveal) var(--ease-enter);
  }
  .seek.edge .seek-cap {
    transform: scaleY(0);
  }
  .seek.edge:hover .seek-cap,
  .seek.edge:focus-visible .seek-cap,
  .seek.edge.dragging .seek-cap {
    transform: none;
  }
  /* Stylized wave (Atmosphere). Edge (main bar): anchored to the bar's edge
     from above — fully collapsed at rest (the flat line carries the progress)
     and grows upward over the content behind when interactive, brightening
     the unplayed portion. Inline (mini) keeps the full wave at rest, growing
     downward from its top. */
  .seek-wave {
    position: relative;
    width: 100%;
    height: 10px;
    transform-origin: top;
    transition:
      opacity var(--dur-reveal) var(--ease-enter),
      transform var(--dur-reveal) var(--ease-enter);
  }
  .seek-wave svg {
    display: block;
    width: 100%;
    height: 100%;
  }
  .seek-wave .wave-base {
    transition: stroke var(--dur-reveal);
  }
  .seek.edge .seek-wave {
    /* 90% = the bar's top edge: the same anchor as .seek-cap's bottom edge,
       so the growing stroke and fill never drift apart. */
    transform-origin: 50% 90%;
    opacity: 0.6;
    transform: scaleY(0);
  }
  .seek.edge:hover .seek-wave,
  .seek.edge:focus-visible .seek-wave,
  .seek.edge.dragging .seek-wave {
    opacity: 1;
    transform: scaleY(1.2);
  }
  .seek.edge:hover .wave-base,
  .seek.edge:focus-visible .wave-base,
  .seek.edge.dragging .wave-base {
    stroke: var(--text-muted);
  }

  /* Floating player bar only (app.css; the rim is display: none elsewhere):
     the line follows the card's top edge round both corners. The rim is the
     card's top band (the bar's corner radius) drawn twice: the track, and the
     fill clipped to the played share of the width, the same mapping the
     pointer uses. Only its top border has width, so each corner's stroke
     tapers from the line's width to nothing as it turns down the side (the
     inner edge becomes an ellipse that meets the outer arc). The line variant
     keeps its thumb (its straight line goes clear). Its top sits on the bar's
     edge (.seek is 1px / 9px above it). */
  .seek.edge {
    /* Where the floating bar's straight edge starts, and the hand-off
       between the rim's corners and the hover wave (below). The wave is
       tapered flat across both (TAPER_FLAT_PX in the script), so the two
       cross-fade at the same height on the bar edge. */
    --corner: var(--radius-sheet);
    --corner-fade: 12px;
  }
  .seek-rim {
    --rim-w: 3px;
    display: none;
    position: absolute;
    left: 0;
    right: 0;
    top: 1px;
    height: var(--corner);
    pointer-events: none;
  }
  .seek.wave .seek-rim {
    --rim-w: 2px;
    top: 9px;
  }
  .rim-track,
  .rim-fill {
    position: absolute;
    inset: 0;
    border: 0 solid var(--line);
    border-top-width: var(--rim-w);
    border-radius: var(--corner) var(--corner) 0 0;
    transition: border-width var(--dur-micro) var(--ease-enter);
  }
  /* The track is well clear of the card's --line side border: the corner
     taper thins to under a pixel, and at a fainter tint the unplayed corner
     vanished, so the line looked cut off before the turn. At this one it
     reads round the corner the way the fill does at the start. */
  .rim-track {
    border-color: color-mix(in srgb, var(--text) 35%, transparent);
  }
  .rim-fill {
    border-color: var(--accent);
  }
  .seek.edge:not(.wave):hover .seek-rim,
  .seek.edge:not(.wave):focus-visible .seek-rim,
  .seek.edge.dragging:not(.wave) .seek-rim {
    --rim-w: 5px;
  }
  /* Wave branch: the wave takes over only along the straight edge. The rim
     keeps its two corners and fades out where the wave fades in; the wave
     lies flat on the edge there and only rises after, so the line runs on
     unbroken from each corner into the wave. */
  .seek.edge.wave:hover .seek-rim,
  .seek.edge.wave:focus-visible .seek-rim,
  .seek.edge.wave.dragging .seek-rim {
    /* Staggered with the wave's fade-in: one of the two is always at full
       strength, so the hand-over has no dim patch. */
    mask-image: linear-gradient(
      to right,
      #000 calc(var(--corner) + var(--corner-fade) / 2),
      transparent calc(var(--corner) + var(--corner-fade)),
      transparent calc(100% - var(--corner) - var(--corner-fade)),
      #000 calc(100% - var(--corner) - var(--corner-fade) / 2)
    );
  }
  /* Same colour as the revealed wave's unplayed stroke (.wave-base). */
  .seek.edge.wave:hover .rim-track,
  .seek.edge.wave:focus-visible .rim-track,
  .seek.edge.wave.dragging .rim-track {
    border-color: var(--text-muted);
  }
  :global(.app[data-player="floating"]) .seek.edge .seek-wave,
  :global(.app[data-player="floating"]) .seek.edge .seek-cap {
    mask-image: linear-gradient(
      to right,
      transparent var(--corner),
      #000 calc(var(--corner) + var(--corner-fade) / 2),
      #000 calc(100% - var(--corner) - var(--corner-fade) / 2),
      transparent calc(100% - var(--corner))
    );
  }
  /* The resting wave is squashed to scaleY(0), but its non-scaling stroke
     survives as a straight line on the bar edge. On the docked bar the flat
     line hides it; here it ran straight past the rim's corners, so it is
     cleared until the wave is revealed. */
  :global(.app[data-player="floating"]) .seek.edge:not(:hover, :focus-visible, .dragging) .seek-wave {
    opacity: 0;
  }
  :global(.app[data-player="floating"]) .seek-rim {
    display: block;
  }
  :global(.app[data-player="floating"]) .seek-flat {
    display: none;
  }
  :global(.app[data-player="floating"]) .seek.edge .seek-line {
    background: transparent;
  }
  :global(.app[data-player="floating"]) .seek.edge .seek-line > .seek-fill {
    visibility: hidden;
  }
</style>
