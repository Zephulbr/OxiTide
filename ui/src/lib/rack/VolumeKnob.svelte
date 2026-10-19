<script lang="ts">
  import type { Player } from "../player.svelte";

  /// Amp volume knob with a 23-dot LED ring (DESIGN §5.7). A slider: drag up
  /// or down, wheel, arrows ±2 %, Page keys ±10 %, Home/End; double-click
  /// mutes. Live while dragging, persisted on release like the VolumeSlider.
  let { player, scale = 1 }: { player: Player; scale?: number } = $props();

  const RING = 23;
  const SWEEP = 270;

  let dragVolume = $state<number | null>(null);
  /// Wheel/key steps land faster than the engine confirms them (sends are
  /// coalesced per frame); step from the last requested value until it does.
  let stepVolume = $state<number | null>(null);
  $effect(() => {
    if (stepVolume !== null && player.state.volume === stepVolume) stepVolume = null;
  });
  const volume = $derived(dragVolume ?? stepVolume ?? player.state.volume);
  const angle = $derived(-SWEEP / 2 + volume * SWEEP);
  const lit = $derived(Math.round(volume * (RING - 1)));
  const dots = Array.from({ length: RING }, (_, i) => {
    const a = ((-SWEEP / 2 + (i * SWEEP) / (RING - 1)) * Math.PI) / 180;
    return { x: 38 + 35 * Math.sin(a), y: 38 - 35 * Math.cos(a) };
  });

  const clamp = (v: number) => Math.min(1, Math.max(0, Math.round(v * 100) / 100));

  let startY = 0;
  let startVolume = 0;
  function onPointerDown(event: PointerEvent) {
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    startY = event.clientY;
    startVolume = volume;
    stepVolume = null;
    dragVolume = startVolume;
  }
  function onPointerMove(event: PointerEvent) {
    if (dragVolume === null) return;
    dragVolume = clamp(startVolume + (startY - event.clientY) / 200);
    void player.setVolume(dragVolume);
  }
  function onPointerUp() {
    if (dragVolume === null) return;
    void player.setVolume(dragVolume, true);
    dragVolume = null;
  }
  function onKeydown(event: KeyboardEvent) {
    const step: Record<string, number> = { ArrowUp: 0.02, ArrowRight: 0.02, ArrowDown: -0.02, ArrowLeft: -0.02, PageUp: 0.1, PageDown: -0.1 };
    let next: number | null = null;
    if (event.key in step) next = clamp(volume + step[event.key]);
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = 1;
    if (next === null) return;
    event.preventDefault();
    stepTo(next);
  }
  function onWheel(event: WheelEvent) {
    event.preventDefault();
    stepTo(clamp(volume - Math.sign(event.deltaY) * 0.02));
  }
  /// Live now, persisted once the steps stop.
  function stepTo(next: number) {
    stepVolume = next;
    void player.setVolume(next, true, true);
  }
</script>

<div class="frame" style:--s={scale}>
<div class="knobbox">
  <svg class="ring" viewBox="0 0 76 76" aria-hidden="true">
    {#each dots as d, i (i)}
      <circle cx={d.x} cy={d.y} r="1.6" class:on={i <= lit && volume > 0} />
    {/each}
  </svg>
  <div
    class="knob"
    role="slider"
    tabindex="0"
    aria-label="Volume"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={Math.round(volume * 100)}
    aria-valuetext={volume === 0 ? "Muted" : `${Math.round(volume * 100)} percent`}
    style:--angle="{angle}deg"
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={() => (dragVolume = null)}
    onkeydown={onKeydown}
    onwheel={onWheel}
    ondblclick={() => player.toggleMute()}
  ></div>
</div>
</div>

<style>
  /* A larger knob is the same part scaled; the frame reserves its real size. */
  .frame {
    width: calc(76px * var(--s));
    height: calc(76px * var(--s));
  }
  .frame > .knobbox {
    transform: scale(var(--s));
    transform-origin: 0 0;
  }
  .knobbox {
    position: relative;
    display: grid;
    place-items: center;
    width: 76px;
    height: 76px;
  }
  .ring {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
  .ring circle {
    fill: #2a1a0a;
    transition: fill 200ms;
  }
  .ring circle.on {
    fill: var(--rack-lamp);
    filter: drop-shadow(0 0 2px var(--rack-lamp-glow));
    transition-duration: 60ms;
  }
  .knob {
    position: relative;
    width: 58px;
    height: 58px;
    border-radius: 50%;
    background: repeating-conic-gradient(from 0deg, #2b2a28 0 2deg, #121110 2deg 4deg);
    box-shadow: 0 6px 12px rgba(0, 0, 0, 0.7), 0 0 0 2px #000;
    cursor: ns-resize;
    touch-action: none;
    transform: rotate(var(--angle));
  }
  .knob::before {
    content: "";
    position: absolute;
    inset: 7px;
    border-radius: 50%;
    background: radial-gradient(circle at 35% 30%, #4a4643, #1c1a18 60%, #0a0908);
    box-shadow: inset 0 1px 1px rgba(255, 255, 255, 0.15), 0 1px 3px rgba(0, 0, 0, 0.8);
  }
  .knob::after {
    content: "";
    position: absolute;
    left: 50%;
    top: 10px;
    width: 3px;
    height: 11px;
    margin-left: -1.5px;
    border-radius: 2px;
    background: var(--rack-lamp);
    box-shadow: 0 0 6px var(--rack-lamp-glow);
  }
  .knob:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 4px;
  }
</style>
