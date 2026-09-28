<script lang="ts">
  import { appearance } from "../appearance.svelte";
  import type { Player } from "../player.svelte";
  import { seekByKey, seekRatio } from "../seek";
  import { wavePath } from "../wave";

  let {
    player,
    variant = "edge",
  }: { player: Player; variant?: "edge" | "inline" } = $props();

  let dragging = $state(false);
  let dragValue = $state(0);

  const shown = $derived(player.pendingSeek ?? player.state.position_ms);
  const position = $derived(dragging ? dragValue : shown);
  const duration = $derived(player.state.duration_ms);
  const progress = $derived((position / Math.max(duration, 1)) * 100);

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
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
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
    class:dragging
    role="slider"
    tabindex="0"
    aria-label="Playback position"
    aria-valuemin={0}
    aria-valuemax={Math.max(duration, 1)}
    aria-valuenow={Math.round(position)}
    onpointerdown={seekPointer}
    onkeydown={seekKeydown}
  >
    {#if appearance.waveSeek}
      {@const pct = (position / Math.max(duration, 1)) * 100}
      {@const d = wavePath(current.track_id, 800, 14)}
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
  /* Edge: hugs the player bar's top border; rests at 4px, grows a touch more
     (+ thumb for the line variant) on hover/focus/drag. */
  .seek.edge {
    position: absolute;
    left: 0;
    right: 0;
    top: -1px;
    height: 12px;
    align-items: flex-start;
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
    transition: height var(--dur-micro) var(--ease-in);
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
    transition: transform var(--dur-micro) var(--ease-in);
  }
  .seek.edge:hover .seek-line,
  .seek.edge:focus-visible .seek-line,
  .seek.edge.dragging .seek-line {
    height: 6px;
  }
  .seek.edge:hover .seek-thumb,
  .seek.edge:focus-visible .seek-thumb,
  .seek.edge.dragging .seek-thumb,
  .seek.inline .seek-thumb {
    transform: translate(-50%, -50%) scale(1);
  }
  /* Stylized wave (Atmosphere): full amplitude + opacity at rest; interactive
     grows it a touch and brightens the unplayed portion. */
  .seek-wave {
    width: 100%;
    height: 10px;
    transform-origin: top;
    transition: transform var(--dur-micro) var(--ease-in);
  }
  .seek-wave svg {
    display: block;
    width: 100%;
    height: 100%;
  }
  .seek-wave .wave-base {
    transition: stroke var(--dur-micro);
  }
  .seek.edge:hover .seek-wave,
  .seek.edge:focus-visible .seek-wave,
  .seek.edge.dragging .seek-wave {
    transform: scaleY(1.3);
  }
  .seek.edge:hover .wave-base,
  .seek.edge:focus-visible .wave-base,
  .seek.edge.dragging .wave-base {
    stroke: var(--text-muted);
  }
</style>
